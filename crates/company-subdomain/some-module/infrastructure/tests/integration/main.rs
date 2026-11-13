//! Integration tests of the example migration and the PostgreSQL repository; each test runs in its
//! own database.

#[cfg(test)]
mod migration;
#[cfg(test)]
mod repository;
