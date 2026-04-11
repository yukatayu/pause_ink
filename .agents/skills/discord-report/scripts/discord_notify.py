#!/usr/bin/env python3
"""Best-effort Discord webhook notifier for a repo-scoped Codex skill.

Configuration resolution order:
1. CODEX_DISCORD_WEBHOOK_URL environment variable
2. <project>/.codex-discord/config.local.json
3. <project>/.codex-discord/config.json

Optional keys in JSON config:
- webhook_url
- author_icon_url
- thumbnail_url
- footer_text
- footer_icon_url
- progress_min_interval_seconds
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, Optional

USERNAME = "お知らせだよ"
AVATAR_URL = "https://yukatayu.tech/files_neko/tayu/%E6%B7%BB%E5%89%8A%E7%B5%90%E6%9E%9C.png"
DEFAULT_PROGRESS_MIN_INTERVAL_SECONDS = 60 * 60
STATE_DIRNAME = ".codex-discord"
STATE_PATHNAME = "state.json"
CONFIG_FILENAMES = ("config.local.json", "config.json")
ACTIVE_TASK_KEY = "active_task"
TASK_BASELINE_TREE_KEY = "baseline_tree"
TASK_STARTED_AT_KEY = "started_at"

SUCCESS_COLOR = 0x57F287
PROGRESS_COLOR = 0xE67E22
PARTIAL_COLOR = 0xFEE75C
BLOCKED_COLOR = 0xED4245
TEST_COLOR = 0x9B59B6
USER_AGENT = "codex-discord-notify/1.0"
RUNNING_FOOTER_TEXT = "実行中"
COMPLETED_FOOTER_TEXT = "完了"
RUNNING_FOOTER_ICON_URL = "https://yukatayu.tech/files_neko/life_quest/status/star_yellow_filled.png"
COMPLETED_FOOTER_ICON_URL = "https://yukatayu.tech/files_neko/life_quest/status/star_green_filled.png"


@dataclass
class ProjectContext:
    working_dir: Path
    project_root: Path
    is_git_repo: bool
    repo_name: str
    branch: Optional[str]
    author_name: str


class NotificationError(Exception):
    """Raised when notification payload construction or delivery fails."""


def eprint(*parts: object) -> None:
    print(*parts, file=sys.stderr)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Best-effort Discord webhook notifier")
    subparsers = parser.add_subparsers(dest="command", required=True)

    common_parent = argparse.ArgumentParser(add_help=False)
    common_parent.add_argument("--summary", default="", help="Short human-readable summary")
    common_parent.add_argument(
        "--cwd",
        default=None,
        help="Working directory to inspect. Defaults to current process directory.",
    )
    common_parent.add_argument(
        "--print-payload",
        action="store_true",
        help="Print the payload JSON to stdout before sending.",
    )

    begin = subparsers.add_parser("begin", help="Record a task baseline without sending a notification")
    begin.add_argument(
        "--cwd",
        default=None,
        help="Working directory to inspect. Defaults to current process directory.",
    )

    progress = subparsers.add_parser("progress", parents=[common_parent], help="Send a progress update")
    progress.add_argument("--next-step", default="", help="Next meaningful milestone")
    progress.add_argument(
        "--min-interval-seconds",
        type=int,
        default=None,
        help="Minimum interval between progress notifications. Defaults to config or 7200.",
    )
    progress.add_argument(
        "--force",
        action="store_true",
        help="Bypass the progress rate limiter for this send.",
    )

    complete = subparsers.add_parser("complete", parents=[common_parent], help="Send a completion update")
    complete.add_argument(
        "--result",
        choices=("success", "partial", "blocked"),
        default="success",
        help="Completion result flavor",
    )
    complete.add_argument(
        "--include-diff",
        action="store_true",
        help="Include aggregated Git diff totals for the current working tree if available.",
    )

    test = subparsers.add_parser("test", parents=[common_parent], help="Send a test notification")
    test.add_argument(
        "--installation-check",
        action="store_true",
        help="Acknowledge that test notifications are only for immediate post-install verification.",
    )

    check = subparsers.add_parser("check", parents=[common_parent], help="Check configuration only")

    return parser.parse_args()


def run_git(args: list[str], cwd: Path, *, env: Optional[Dict[str, str]] = None) -> Optional[str]:
    try:
        result = subprocess.run(
            ["git", *args],
            cwd=str(cwd),
            env=env,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
    except (subprocess.CalledProcessError, FileNotFoundError):
        return None
    return result.stdout.strip()


def detect_project_context(cwd_arg: Optional[str]) -> ProjectContext:
    working_dir = Path(cwd_arg or os.getcwd()).resolve()
    git_root = run_git(["rev-parse", "--show-toplevel"], working_dir)

    if git_root:
        project_root = Path(git_root).resolve()
        repo_name = project_root.name
        branch = run_git(["branch", "--show-current"], working_dir)
        if not branch:
            sha = run_git(["rev-parse", "--short", "HEAD"], working_dir)
            branch = f"detached@{sha}" if sha else "detached"
        author_name = f"{repo_name}:{branch}"
        return ProjectContext(
            working_dir=working_dir,
            project_root=project_root,
            is_git_repo=True,
            repo_name=repo_name,
            branch=branch,
            author_name=author_name,
        )

    folder_name = working_dir.name or str(working_dir)
    return ProjectContext(
        working_dir=working_dir,
        project_root=working_dir,
        is_git_repo=False,
        repo_name=folder_name,
        branch=None,
        author_name=folder_name,
    )


def load_json(path: Path) -> Dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return {}
    except json.JSONDecodeError as exc:
        raise NotificationError(f"Invalid JSON in {path}: {exc}") from exc


def config_dir(project_root: Path) -> Path:
    return project_root / STATE_DIRNAME


def load_config(project_root: Path) -> Dict[str, Any]:
    merged: Dict[str, Any] = {}
    cfg_dir = config_dir(project_root)
    for name in reversed(CONFIG_FILENAMES):
        merged.update(load_json(cfg_dir / name))
    webhook = os.environ.get("CODEX_DISCORD_WEBHOOK_URL")
    if webhook:
        merged["webhook_url"] = webhook
    return merged


def state_path(project_root: Path) -> Path:
    return config_dir(project_root) / STATE_PATHNAME


def load_state(project_root: Path) -> Dict[str, Any]:
    return load_json(state_path(project_root))


def save_state(project_root: Path, state: Dict[str, Any]) -> None:
    path = state_path(project_root)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(state, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def load_active_task(project_root: Path) -> Optional[Dict[str, Any]]:
    state = load_state(project_root)
    active_task = state.get(ACTIVE_TASK_KEY)
    return active_task if isinstance(active_task, dict) else None


def save_active_task(project_root: Path, task: Dict[str, Any]) -> None:
    state = load_state(project_root)
    state[ACTIVE_TASK_KEY] = task
    state.pop("last_progress_sent_at", None)
    save_state(project_root, state)


def clear_active_task(project_root: Path) -> None:
    state = load_state(project_root)
    if ACTIVE_TASK_KEY in state:
        del state[ACTIVE_TASK_KEY]
        save_state(project_root, state)


def capture_worktree_tree(ctx: ProjectContext) -> Optional[str]:
    if not ctx.is_git_repo:
        return None

    with tempfile.TemporaryDirectory(prefix="codex-discord-index-") as td:
        index_path = Path(td) / "index"
        env = os.environ.copy()
        env["GIT_INDEX_FILE"] = str(index_path)

        staged = run_git(
            ["add", "-A", "--", ".", ":(exclude).codex-discord", ":(exclude).codex-discord/**"],
            ctx.project_root,
            env=env,
        )
        if staged is None:
            return None

        tree = run_git(["write-tree"], ctx.project_root, env=env)
        if not tree:
            return None
        return tree


def begin_task(ctx: ProjectContext) -> None:
    task: Dict[str, Any] = {
        TASK_STARTED_AT_KEY: time.time(),
    }
    baseline_tree = capture_worktree_tree(ctx)
    if baseline_tree:
        task[TASK_BASELINE_TREE_KEY] = baseline_tree
    save_active_task(ctx.project_root, task)


def parse_numstat(output: str) -> tuple[int, int]:
    added = 0
    removed = 0
    for raw_line in output.splitlines():
        parts = raw_line.split("\t", 2)
        if len(parts) < 2:
            continue
        a, d = parts[0], parts[1]
        if a == "-" or d == "-":
            continue
        try:
            added += int(a)
            removed += int(d)
        except ValueError:
            continue
    return added, removed


def diff_totals(ctx: ProjectContext) -> Optional[tuple[int, int]]:
    active_task = load_active_task(ctx.project_root)
    if not active_task:
        return None

    baseline_tree = active_task.get(TASK_BASELINE_TREE_KEY)
    if not isinstance(baseline_tree, str) or not baseline_tree.strip():
        return None

    current_tree = capture_worktree_tree(ctx)
    if not current_tree:
        return None

    output = run_git(["diff", "--numstat", baseline_tree.strip(), current_tree, "--"], ctx.project_root)
    if output is None:
        return None

    return parse_numstat(output)


def progress_allowed(project_root: Path, min_interval_seconds: int) -> bool:
    state = load_state(project_root)
    last_sent = state.get("last_progress_sent_at")
    if not isinstance(last_sent, (int, float)):
        return True
    return (time.time() - float(last_sent)) >= min_interval_seconds


def mark_progress_sent(project_root: Path) -> None:
    state = load_state(project_root)
    state["last_progress_sent_at"] = time.time()
    save_state(project_root, state)


def trim(text: str, limit: int) -> str:
    text = text.strip()
    if len(text) <= limit:
        return text
    return text[: limit - 1].rstrip() + "…"


def build_embed(
    *,
    command: str,
    summary: str,
    ctx: ProjectContext,
    config: Dict[str, Any],
    next_step: str = "",
    include_diff: bool = False,
    result: str = "success",
) -> Dict[str, Any]:
    summary = trim(summary, 1500)
    next_step = trim(next_step, 300)

    if command == "progress":
        color = PROGRESS_COLOR
        description = summary
    elif command == "complete":
        color_map = {
            "success": SUCCESS_COLOR,
            "partial": PARTIAL_COLOR,
            "blocked": BLOCKED_COLOR,
        }
        color = color_map[result]
        description = summary
    elif command == "test":
        title = "疎通確認"
        color = TEST_COLOR
        description = "Discord Webhook への送信テストです。"
        if summary:
            description += f"\n\n{summary}"
    else:
        raise NotificationError(f"Unsupported command for embed: {command}")

    embed: Dict[str, Any] = {
        "author": {"name": trim(ctx.author_name, 256)},
        "color": color,
        "timestamp": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    }
    if command == "test":
        embed["title"] = title
    if description:
        embed["description"] = trim(description, 4096)

    author_icon_url = config.get("author_icon_url")
    if isinstance(author_icon_url, str) and author_icon_url.strip():
        embed["author"]["icon_url"] = author_icon_url.strip()

    thumbnail_url = config.get("thumbnail_url")
    if isinstance(thumbnail_url, str) and thumbnail_url.strip():
        embed["thumbnail"] = {"url": thumbnail_url.strip()}

    fields = []

    if command == "progress" and next_step:
        fields.append({"name": "Next", "value": trim(next_step, 1024), "inline": False})

    if command == "complete" and include_diff:
        totals = diff_totals(ctx)
        if totals is not None:
            added, removed = totals
            fields.append(
                {
                    "name": "変更量",
                    "value": f"+{added} / -{removed}",
                    "inline": True,
                }
            )

    if fields:
        embed["fields"] = fields[:25]

    footer_text = config.get("footer_text")
    footer_icon_url = config.get("footer_icon_url")
    if command == "progress":
        embed["footer"] = {"text": RUNNING_FOOTER_TEXT, "icon_url": RUNNING_FOOTER_ICON_URL}
    elif command == "complete":
        embed["footer"] = {"text": COMPLETED_FOOTER_TEXT, "icon_url": COMPLETED_FOOTER_ICON_URL}
    elif isinstance(footer_text, str) and footer_text.strip():
        embed["footer"] = {"text": trim(footer_text, 2048)}
        if isinstance(footer_icon_url, str) and footer_icon_url.strip():
            embed["footer"]["icon_url"] = footer_icon_url.strip()

    return embed


def build_payload(embed: Dict[str, Any]) -> Dict[str, Any]:
    return {
        "username": USERNAME,
        "avatar_url": AVATAR_URL,
        "embeds": [embed],
        "allowed_mentions": {"parse": []},
    }


def post_webhook(webhook_url: str, payload: Dict[str, Any]) -> None:
    body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
    request = urllib.request.Request(
        webhook_url,
        data=body,
        headers={
            "Content-Type": "application/json",
            "User-Agent": USER_AGENT,
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=15) as response:
            response.read()
    except urllib.error.HTTPError as exc:
        detail = exc.read().decode("utf-8", errors="replace")
        raise NotificationError(f"Discord webhook HTTP {exc.code}: {detail}") from exc
    except urllib.error.URLError as exc:
        raise NotificationError(f"Discord webhook connection error: {exc}") from exc


def command_check(config: Dict[str, Any]) -> int:
    webhook_url = config.get("webhook_url")
    if not isinstance(webhook_url, str) or not webhook_url.strip():
        eprint("Webhook is not configured. Set CODEX_DISCORD_WEBHOOK_URL or create .codex-discord/config.local.json")
        return 1
    print("Webhook configuration detected.")
    return 0


def main() -> int:
    args = parse_args()
    ctx = detect_project_context(args.cwd)
    config = load_config(ctx.project_root)

    if args.command == "check":
        return command_check(config)

    if args.command == "begin":
        begin_task(ctx)
        print("Task baseline recorded.")
        return 0

    webhook_url = config.get("webhook_url")
    if not isinstance(webhook_url, str) or not webhook_url.strip():
        eprint("Webhook is not configured; skipping Discord send (best effort).")
        return 0

    if args.command == "progress":
        configured_min = config.get("progress_min_interval_seconds")
        min_interval = args.min_interval_seconds
        if min_interval is None:
            if isinstance(configured_min, int):
                min_interval = configured_min
            else:
                min_interval = DEFAULT_PROGRESS_MIN_INTERVAL_SECONDS
        if not args.force and not progress_allowed(ctx.project_root, min_interval):
            eprint(f"Progress notification skipped: last send was within {min_interval} seconds.")
            return 0
        embed = build_embed(
            command="progress",
            summary=args.summary,
            ctx=ctx,
            config=config,
            next_step=args.next_step,
        )
        payload = build_payload(embed)
        if args.print_payload:
            print(json.dumps(payload, ensure_ascii=False, indent=2))
        post_webhook(webhook_url.strip(), payload)
        mark_progress_sent(ctx.project_root)
        print("Progress notification sent.")
        return 0

    if args.command == "complete":
        embed = build_embed(
            command="complete",
            summary=args.summary,
            ctx=ctx,
            config=config,
            include_diff=args.include_diff,
            result=args.result,
        )
        payload = build_payload(embed)
        if args.print_payload:
            print(json.dumps(payload, ensure_ascii=False, indent=2))
        post_webhook(webhook_url.strip(), payload)
        clear_active_task(ctx.project_root)
        print("Completion notification sent.")
        return 0

    if args.command == "test":
        if not args.installation_check:
            eprint("Test notifications are only for 導入直後の疎通確認です。実運用では送らないでください。")
            return 1
        embed = build_embed(
            command="test",
            summary=args.summary,
            ctx=ctx,
            config=config,
        )
        payload = build_payload(embed)
        if args.print_payload:
            print(json.dumps(payload, ensure_ascii=False, indent=2))
        post_webhook(webhook_url.strip(), payload)
        print("Test notification sent.")
        return 0

    raise NotificationError(f"Unsupported command: {args.command}")


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except NotificationError as exc:
        eprint(f"Notification error: {exc}")
        raise SystemExit(1)
