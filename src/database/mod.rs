mod cork;
mod database;
mod de;
mod deserialized;
mod engine;
mod handle;
pub mod keyval;
mod map;
pub mod maps;
mod opts;
mod ser;
mod stream;
mod util;
mod watchers;

pub(crate) use self::{
	engine::Engine,
	util::{or_else, result},
};

extern crate conduit_core as conduit;
extern crate rust_rocksdb as rocksdb;

pub use self::{
	database::Database,
	de::Ignore,
	deserialized::Deserialized,
	handle::Handle,
	keyval::{KeyVal, Slice},
	map::Map,
	ser::{Interfix, Separator},
};

conduit::mod_ctor! {}
conduit::mod_dtor! {}
conduit::rustc_flags_capture! {}
