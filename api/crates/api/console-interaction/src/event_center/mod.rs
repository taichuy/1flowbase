//! Host-owned declaration reconciliation and runtime observation; business rules stay in control-plane.
pub(crate) mod business;
pub mod catalog;
pub(crate) mod interface;

pub(crate) mod api_observer;

pub(crate) mod api_catalog;

pub(crate) mod persistence;
