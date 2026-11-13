//! The steps that make a dependency fail.

use cucumber::given;
use health_check_domain::Dependency;

use crate::world::AppWorld;

#[given("the cache is not working")]
fn cache_is_not_working(world: &mut AppWorld) {
    world.fail(Dependency::Cache);
}

#[given("the database is not available")]
fn database_is_not_available(world: &mut AppWorld) {
    world.fail(Dependency::Database);
}

#[given("the message broker is not available")]
fn message_broker_is_not_available(world: &mut AppWorld) {
    world.fail(Dependency::MessageBroker);
}
