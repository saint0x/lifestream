use serde_json::Value;

use crate::obs::domain::{
    InstanceInput, InstancePatch, SceneGroupInput, SceneGroupPatch, SceneInput, ScenePatch,
    SceneReorderInput, SceneTemplateInput, TransitionPreviewInput,
};

use super::{
    ObsService, ObsServiceResult, TRANSITION_KINDS, VALIDATION_STATES, require_one_of,
    require_positive, require_text,
};

impl ObsService {
    pub async fn create_scene(&self, input: SceneInput) -> ObsServiceResult<Value> {
        validate_scene_input(&input)?;
        Ok(self.store.create_scene(input).await?)
    }

    pub async fn patch_scene(&self, scene_id: &str, input: ScenePatch) -> ObsServiceResult<Value> {
        require_text(scene_id, "scene_id")?;
        if let Some(name) = input.name.as_deref() {
            require_text(name, "name")?;
        }
        if let Some(kind) = input.transition_kind.as_deref() {
            require_one_of(kind, "transition_kind", TRANSITION_KINDS)?;
        }
        if let Some(state) = input.validation_state.as_deref() {
            require_one_of(state, "validation_state", VALIDATION_STATES)?;
        }
        Ok(self.store.patch_scene(scene_id, input).await?)
    }

    pub async fn delete_scene(&self, scene_id: &str) -> ObsServiceResult<Value> {
        require_text(scene_id, "scene_id")?;
        Ok(self.store.delete_scene(scene_id).await?)
    }

    pub async fn reorder_scenes(
        &self,
        collection_id: &str,
        input: SceneReorderInput,
    ) -> ObsServiceResult<Value> {
        require_text(collection_id, "collection_id")?;
        if input.scene_ids.is_empty() {
            return Err(super::ObsServiceError::Invalid {
                field: "scene_ids",
                message: "must include every scene in the collection",
            });
        }
        for scene_id in &input.scene_ids {
            require_text(scene_id, "scene_id")?;
        }
        Ok(self
            .store
            .reorder_scenes(collection_id, input.scene_ids)
            .await?)
    }

    pub async fn scene_templates(&self) -> ObsServiceResult<Vec<Value>> {
        Ok(self.store.scene_templates().await?)
    }

    pub async fn create_scene_from_template(
        &self,
        template_id: &str,
        input: SceneTemplateInput,
    ) -> ObsServiceResult<Value> {
        require_text(template_id, "template_id")?;
        require_text(&input.collection_id, "collection_id")?;
        if let Some(name) = input.name.as_deref() {
            require_text(name, "name")?;
        }
        Ok(self
            .store
            .create_scene_from_template(template_id, input)
            .await?)
    }

    pub async fn duplicate_scene(&self, scene_id: &str) -> ObsServiceResult<Value> {
        require_text(scene_id, "scene_id")?;
        Ok(self.store.duplicate_scene(scene_id).await?)
    }

    pub async fn send_to_program(&self, scene_id: &str) -> ObsServiceResult<Value> {
        require_text(scene_id, "scene_id")?;
        Ok(self.store.send_to_program(scene_id).await?)
    }

    pub async fn transition_preview(
        &self,
        scene_id: &str,
        input: TransitionPreviewInput,
    ) -> ObsServiceResult<Value> {
        require_text(scene_id, "scene_id")?;
        if let Some(from_scene_id) = input.from_scene_id.as_deref() {
            require_text(from_scene_id, "from_scene_id")?;
        }
        Ok(self.store.transition_preview(scene_id, input).await?)
    }

    pub async fn create_instance(
        &self,
        scene_id: &str,
        input: InstanceInput,
    ) -> ObsServiceResult<Value> {
        require_text(scene_id, "scene_id")?;
        require_text(&input.source_id, "source_id")?;
        require_positive(input.width, "width")?;
        require_positive(input.height, "height")?;
        Ok(self.store.create_instance(scene_id, input).await?)
    }

    pub async fn create_scene_group(
        &self,
        scene_id: &str,
        input: SceneGroupInput,
    ) -> ObsServiceResult<Value> {
        require_text(scene_id, "scene_id")?;
        require_text(&input.child_scene_id, "child_scene_id")?;
        require_text(&input.label, "label")?;
        if let Some(width) = input.width {
            require_positive(width, "width")?;
        }
        if let Some(height) = input.height {
            require_positive(height, "height")?;
        }
        Ok(self.store.create_scene_group(scene_id, input).await?)
    }

    pub async fn patch_scene_group(
        &self,
        source_id: &str,
        input: SceneGroupPatch,
    ) -> ObsServiceResult<Value> {
        require_text(source_id, "source_id")?;
        if let Some(scene_id) = input.child_scene_id.as_deref() {
            require_text(scene_id, "child_scene_id")?;
        }
        if let Some(label) = input.label.as_deref() {
            require_text(label, "label")?;
        }
        Ok(self.store.patch_scene_group(source_id, input).await?)
    }

    pub async fn patch_instance(
        &self,
        instance_id: &str,
        input: InstancePatch,
    ) -> ObsServiceResult<Value> {
        require_text(instance_id, "instance_id")?;
        if let Some(width) = input.width {
            require_positive(width, "width")?;
        }
        if let Some(height) = input.height {
            require_positive(height, "height")?;
        }
        Ok(self.store.patch_instance(instance_id, input).await?)
    }
}

fn validate_scene_input(input: &SceneInput) -> ObsServiceResult<()> {
    require_text(&input.collection_id, "collection_id")?;
    require_text(&input.name, "name")?;
    if let Some(kind) = input.transition_kind.as_deref() {
        require_one_of(kind, "transition_kind", TRANSITION_KINDS)?;
    }
    Ok(())
}
