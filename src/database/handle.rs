use std::{fmt, fmt::Debug, ops::Deref};

use conduit::Result;
use rocksdb::DBPinnableSlice;
use serde::{Deserialize, Serialize, Serializer};

use crate::{keyval::deserialize_val, Deserialized, Slice};

pub struct Handle<'a> {
	val: DBPinnableSlice<'a>,
}

impl<'a> From<DBPinnableSlice<'a>> for Handle<'a> {
	fn from(val: DBPinnableSlice<'a>) -> Self {
		Self {
			val,
		}
	}
}

impl Debug for Handle<'_> {
	fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
		let val: &Slice = self;
		let ptr = val.as_ptr();
		let len = val.len();
		write!(out, "Handle {{val: {{ptr: {ptr:?}, len: {len}}}}}")
	}
}

impl Serialize for Handle<'_> {
	#[inline]
	fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		let bytes: &Slice = self;
		serializer.serialize_bytes(bytes)
	}
}

impl Deref for Handle<'_> {
	type Target = Slice;

	#[inline]
	fn deref(&self) -> &Self::Target { &self.val }
}

impl AsRef<Slice> for Handle<'_> {
	#[inline]
	fn as_ref(&self) -> &Slice { &self.val }
}

impl Deserialized for Result<Handle<'_>> {
	#[inline]
	fn map_json<T, U, F>(self, f: F) -> Result<U>
	where
		F: FnOnce(T) -> U,
		T: for<'de> Deserialize<'de>,
	{
		self?.map_json(f)
	}

	#[inline]
	fn map_de<T, U, F>(self, f: F) -> Result<U>
	where
		F: FnOnce(T) -> U,
		T: for<'de> Deserialize<'de>,
	{
		self?.map_de(f)
	}
}

impl<'a> Deserialized for Result<&'a Handle<'a>> {
	#[inline]
	fn map_json<T, U, F>(self, f: F) -> Result<U>
	where
		F: FnOnce(T) -> U,
		T: for<'de> Deserialize<'de>,
	{
		self.and_then(|handle| handle.map_json(f))
	}

	#[inline]
	fn map_de<T, U, F>(self, f: F) -> Result<U>
	where
		F: FnOnce(T) -> U,
		T: for<'de> Deserialize<'de>,
	{
		self.and_then(|handle| handle.map_de(f))
	}
}

impl<'a> Deserialized for &'a Handle<'a> {
	fn map_json<T, U, F>(self, f: F) -> Result<U>
	where
		F: FnOnce(T) -> U,
		T: for<'de> Deserialize<'de>,
	{
		serde_json::from_slice::<T>(self.as_ref())
			.map_err(Into::into)
			.map(f)
	}

	fn map_de<T, U, F>(self, f: F) -> Result<U>
	where
		F: FnOnce(T) -> U,
		T: for<'de> Deserialize<'de>,
	{
		deserialize_val(self.as_ref()).map(f)
	}
}
