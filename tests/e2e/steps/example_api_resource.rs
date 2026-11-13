//! The step that creates an example API resource through the API.

use cucumber::given;
use serde_json::json;

use crate::world::AppWorld;

#[given(expr = "an example API resource exists with name {string} and description {string}")]
async fn resource_exists(world: &mut AppWorld, name: String, description: String) {
    let body = json!({"name": name, "description": description}).to_string();
    world
        .send(
            "POST",
            "/api/example_api_resources",
            Some("application/json"),
            &body,
        )
        .await;
    assert_eq!(world.reply().status, 201, "body: {}", world.reply().body);
}
