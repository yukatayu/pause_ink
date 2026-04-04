use crate::{
    AnnotationProject, ClearEvent, ClearEventId, Command, CommandError, GlyphObject, GlyphObjectId,
    Group, GroupId, Stroke, StrokeId,
};

pub struct InsertStrokeCommand {
    pub stroke: Stroke,
    pub index: Option<usize>,
}

impl Command<AnnotationProject> for InsertStrokeCommand {
    fn apply(&self, state: &mut AnnotationProject) -> Result<(), CommandError> {
        if state.stroke_index(&self.stroke.id).is_some() {
            return Err(CommandError::new(format!(
                "stroke already exists: {}",
                self.stroke.id.0
            )));
        }

        let index = self.index.unwrap_or(state.strokes.len()).min(state.strokes.len());
        state.strokes.insert(index, self.stroke.clone());
        Ok(())
    }

    fn undo(&self, state: &mut AnnotationProject) -> Result<(), CommandError> {
        remove_by_id(&mut state.strokes, &self.stroke.id, |stroke| &stroke.id)
    }
}

pub struct InsertGlyphObjectCommand {
    pub object: GlyphObject,
    pub index: Option<usize>,
}

impl Command<AnnotationProject> for InsertGlyphObjectCommand {
    fn apply(&self, state: &mut AnnotationProject) -> Result<(), CommandError> {
        if state.glyph_object_index(&self.object.id).is_some() {
            return Err(CommandError::new(format!(
                "glyph object already exists: {}",
                self.object.id.0
            )));
        }

        let index = self
            .index
            .unwrap_or(state.glyph_objects.len())
            .min(state.glyph_objects.len());
        state.glyph_objects.insert(index, self.object.clone());
        Ok(())
    }

    fn undo(&self, state: &mut AnnotationProject) -> Result<(), CommandError> {
        remove_by_id(
            &mut state.glyph_objects,
            &self.object.id,
            |object| &object.id,
        )
    }
}

pub struct InsertGroupCommand {
    pub group: Group,
    pub index: Option<usize>,
}

impl Command<AnnotationProject> for InsertGroupCommand {
    fn apply(&self, state: &mut AnnotationProject) -> Result<(), CommandError> {
        if state.group_index(&self.group.id).is_some() {
            return Err(CommandError::new(format!(
                "group already exists: {}",
                self.group.id.0
            )));
        }

        let index = self.index.unwrap_or(state.groups.len()).min(state.groups.len());
        state.groups.insert(index, self.group.clone());
        Ok(())
    }

    fn undo(&self, state: &mut AnnotationProject) -> Result<(), CommandError> {
        remove_by_id(&mut state.groups, &self.group.id, |group| &group.id)
    }
}

pub struct InsertClearEventCommand {
    pub clear_event: ClearEvent,
    pub index: Option<usize>,
}

impl Command<AnnotationProject> for InsertClearEventCommand {
    fn apply(&self, state: &mut AnnotationProject) -> Result<(), CommandError> {
        if state.clear_event_index(&self.clear_event.id).is_some() {
            return Err(CommandError::new(format!(
                "clear event already exists: {}",
                self.clear_event.id.0
            )));
        }

        let index = self
            .index
            .unwrap_or(state.clear_events.len())
            .min(state.clear_events.len());
        state.clear_events.insert(index, self.clear_event.clone());
        Ok(())
    }

    fn undo(&self, state: &mut AnnotationProject) -> Result<(), CommandError> {
        remove_by_id(
            &mut state.clear_events,
            &self.clear_event.id,
            |clear_event| &clear_event.id,
        )
    }
}

pub struct SetGlyphObjectZIndexCommand {
    pub object_id: GlyphObjectId,
    pub from: i32,
    pub to: i32,
}

impl Command<AnnotationProject> for SetGlyphObjectZIndexCommand {
    fn apply(&self, state: &mut AnnotationProject) -> Result<(), CommandError> {
        let object = find_object_mut(state, &self.object_id)?;
        if object.ordering.z_index != self.from {
            return Err(CommandError::new(format!(
                "unexpected current z-index for {}: expected {}, got {}",
                self.object_id.0, self.from, object.ordering.z_index
            )));
        }
        object.ordering.z_index = self.to;
        Ok(())
    }

    fn undo(&self, state: &mut AnnotationProject) -> Result<(), CommandError> {
        let object = find_object_mut(state, &self.object_id)?;
        if object.ordering.z_index != self.to {
            return Err(CommandError::new(format!(
                "unexpected current z-index for {} during undo: expected {}, got {}",
                self.object_id.0, self.to, object.ordering.z_index
            )));
        }
        object.ordering.z_index = self.from;
        Ok(())
    }
}

pub struct AppendStrokeToGlyphObjectCommand {
    pub object_id: GlyphObjectId,
    pub stroke_id: StrokeId,
}

impl Command<AnnotationProject> for AppendStrokeToGlyphObjectCommand {
    fn apply(&self, state: &mut AnnotationProject) -> Result<(), CommandError> {
        let object = find_object_mut(state, &self.object_id)?;
        if object.stroke_ids.iter().any(|stroke_id| stroke_id == &self.stroke_id) {
            return Err(CommandError::new(format!(
                "stroke {} is already attached to glyph object {}",
                self.stroke_id.0, self.object_id.0
            )));
        }
        object.stroke_ids.push(self.stroke_id.clone());
        Ok(())
    }

    fn undo(&self, state: &mut AnnotationProject) -> Result<(), CommandError> {
        let object = find_object_mut(state, &self.object_id)?;
        let Some(index) = object
            .stroke_ids
            .iter()
            .position(|stroke_id| stroke_id == &self.stroke_id)
        else {
            return Err(CommandError::new(format!(
                "stroke {} is not attached to glyph object {}",
                self.stroke_id.0, self.object_id.0
            )));
        };
        object.stroke_ids.remove(index);
        Ok(())
    }
}

fn remove_by_id<T, Id, F>(items: &mut Vec<T>, target_id: &Id, id_of: F) -> Result<(), CommandError>
where
    Id: PartialEq + std::fmt::Display,
    F: Fn(&T) -> &Id,
{
    let Some(index) = items.iter().position(|item| id_of(item) == target_id) else {
        return Err(CommandError::new(format!("entity not found during undo: {target_id}")));
    };
    items.remove(index);
    Ok(())
}

fn find_object_mut<'a>(
    state: &'a mut AnnotationProject,
    object_id: &GlyphObjectId,
) -> Result<&'a mut GlyphObject, CommandError> {
    state
        .glyph_objects
        .iter_mut()
        .find(|object| object.id == *object_id)
        .ok_or_else(|| CommandError::new(format!("glyph object not found: {}", object_id.0)))
}

impl std::fmt::Display for StrokeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::fmt::Display for GlyphObjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::fmt::Display for GroupId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::fmt::Display for ClearEventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
