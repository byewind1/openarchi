//! Original synthetic input, shared by UI tests on every desktop platform.
use super::*;
use serde_json::{Value, json};
pub(super) fn space_plan(page: &crate::drawings::SourcePage) -> Value {
    let seed = |name: &str, x: f64, w: f64| json!({"name":name,"spec":{"purpose":"controlled concept, not approved","outline":[{"x":x,"y":30.},{"x":x+w,"y":30.},{"x":x+w,"y":34.},{"x":x,"y":34.}],"holes":[],"height":3.,"bottom_offset":0.,"label_position":null}});
    json!({"task_id":storage::new_document_identity(),"authorship":"public typed fixture, not AI interpretation","draft":{"name":"Intent product path","question":"Create editable concepts with unknown dimensions; preserve previous work","expected_result":"two editable space candidates","images":[{"page":page.id,"sha256":page.sha256}],"claims":[{"kind":"design_completion","text":"Test rooms and scale are explicitly authored assumptions"}],"geometry_assumptions":"All sizes/height/placement assumed; no survey/code conclusion","statements":[],"candidate_names":["Intent one","Intent two"],"candidates":[[seed("Room",30.,6.),seed("Shared",36.,3.)],[seed("Room",30.,7.),seed("Shared",37.,2.)]],"story":0,"layer":0}})
}
pub(super) fn mass_plan(page: &crate::drawings::SourcePage) -> Value {
    let mut plan = space_plan(page);
    plan["draft"]["name"] = json!("Mass intent product path");
    plan["draft"]["expected_result"] = json!("two editable concept mass candidates");
    for group in plan["draft"]["candidates"].as_array_mut().unwrap() {
        for (index, seed) in group.as_array_mut().unwrap().iter_mut().enumerate() {
            seed["name"] = json!(format!("Mass {}", index + 1));
            seed["spec"].as_object_mut().unwrap().remove("purpose");
            seed["spec"]
                .as_object_mut()
                .unwrap()
                .remove("label_position");
            seed["material"] = json!("plaster");
        }
    }
    plan
}
