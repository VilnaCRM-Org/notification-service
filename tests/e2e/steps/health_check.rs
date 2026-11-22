//! The steps that make a dependency fail.

use cucumber::given;
use health_check_domain::Dependency;

use crate::world::AppWorld;

#[given("the cache is not working")]
fn cache_is_not_working(world: &mut AppWorld) {
    world.fail(Dependency::Cache);
}
