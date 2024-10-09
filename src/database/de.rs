use conduit::{checked, debug::DebugInspect, err, utils::string, Error, Result};
use serde::{
	de,
	de::{DeserializeSeed, Visitor},
	Deserialize,
};

/// Deserialize into T from buffer.
pub(crate) fn from_slice<'a, T>(buf: &'a [u8]) -> Result<T>
where
	T: Deserialize<'a>,
{
	let mut deserializer = Deserializer {
		buf,
		pos: 0,
		seq: false,
	};

	T::deserialize(&mut deserializer).debug_inspect(|_| {
		deserializer
			.finished()
			.expect("deserialization failed to consume trailing bytes");
	})
}

/// Deserialization state.
pub(crate) struct Deserializer<'de> {
	buf: &'de [u8],
	pos: usize,
	seq: bool,
}

/// Directive to ignore a record. This type can be used to skip deserialization
/// until the next separator is found.
#[derive(Debug, Deserialize)]
pub struct Ignore;

/// Directive to ignore all remaining records. This can be used in a sequence to
/// ignore the rest of the sequence.
#[derive(Debug, Deserialize)]
pub struct IgnoreAll;

impl<'de> Deserializer<'de> {
	/// Record separator; an intentionally invalid-utf8 byte.
	const SEP: u8 = b'\xFF';

	/// Determine if the input was fully consumed and error if bytes remaining.
	/// This is intended for debug assertions; not optimized for parsing logic.
	fn finished(&self) -> Result<()> {
		let pos = self.pos;
		let len = self.buf.len();
		let parsed = &self.buf[0..pos];
		let unparsed = &self.buf[pos..];
		let remain = checked!(len - pos)?;
		let trailing_sep = remain == 1 && unparsed[0] == Self::SEP;
		(remain == 0 || trailing_sep)
			.then_some(())
			.ok_or(err!(SerdeDe(
				"{remain} trailing of {len} bytes not deserialized.\n{parsed:?}\n{unparsed:?}",
			)))
	}

	/// Called at the start of arrays and tuples
	#[inline]
	fn sequence_start(&mut self) {
		debug_assert!(!self.seq, "Nested sequences are not handled at this time");
		self.seq = true;
	}

	/// Consume the current record to ignore it. Inside a sequence the next
	/// record is skipped but at the top-level all records are skipped such that
	/// deserialization completes with self.finished() == Ok.
	#[inline]
	fn record_ignore(&mut self) {
		if self.seq {
			self.record_next();
		} else {
			self.record_ignore_all();
		}
	}

	/// Consume the current and all remaining records to ignore them. Similar to
	/// Ignore at the top-level, but it can be provided in a sequence to Ignore
	/// all remaining elements.
	#[inline]
	fn record_ignore_all(&mut self) { self.record_trail(); }

	/// Consume the current record. The position pointer is moved to the start
	/// of the next record. Slice of the current record is returned.
	#[inline]
	fn record_next(&mut self) -> &'de [u8] {
		self.buf[self.pos..]
			.split(|b| *b == Deserializer::SEP)
			.inspect(|record| self.inc_pos(record.len()))
			.next()
			.expect("remainder of buf even if SEP was not found")
	}

	/// Peek at the first byte of the current record. If all records were
	/// consumed None is returned instead.
	#[inline]
	fn record_peek_byte(&self) -> Option<u8> {
		let started = self.pos != 0;
		let buf = &self.buf[self.pos..];
		debug_assert!(
			!started || buf[0] == Self::SEP,
			"Missing expected record separator at current position"
		);

		buf.get::<usize>(started.into()).copied()
	}

	/// Consume the record separator such that the position cleanly points to
	/// the start of the next record. (Case for some sequences)
	#[inline]
	fn record_start(&mut self) {
		let started = self.pos != 0;
		debug_assert!(
			!started || self.buf[self.pos] == Self::SEP,
			"Missing expected record separator at current position"
		);

		self.inc_pos(started.into());
	}

	/// Consume all remaining bytes, which may include record separators,
	/// returning a raw slice.
	#[inline]
	fn record_trail(&mut self) -> &'de [u8] {
		let record = &self.buf[self.pos..];
		self.inc_pos(record.len());
		record
	}

	/// Increment the position pointer.
	#[inline]
	fn inc_pos(&mut self, n: usize) {
		self.pos = self.pos.saturating_add(n);
		debug_assert!(self.pos <= self.buf.len(), "pos out of range");
	}
}

impl<'a, 'de: 'a> de::Deserializer<'de> for &'a mut Deserializer<'de> {
	type Error = Error;

	fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
	where
		V: Visitor<'de>,
	{
		self.sequence_start();
		visitor.visit_seq(self)
	}

	fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value>
	where
		V: Visitor<'de>,
	{
		self.sequence_start();
		visitor.visit_seq(self)
	}

	fn deserialize_tuple_struct<V>(self, _name: &'static str, _len: usize, visitor: V) -> Result<V::Value>
	where
		V: Visitor<'de>,
	{
		self.sequence_start();
		visitor.visit_seq(self)
	}

	fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
	where
		V: Visitor<'de>,
	{
		let input = self.record_next();
		let mut d = serde_json::Deserializer::from_slice(input);
		d.deserialize_map(visitor).map_err(Into::into)
	}

	fn deserialize_struct<V>(self, name: &'static str, fields: &'static [&'static str], visitor: V) -> Result<V::Value>
	where
		V: Visitor<'de>,
	{
		let input = self.record_next();
		let mut d = serde_json::Deserializer::from_slice(input);
		d.deserialize_struct(name, fields, visitor)
			.map_err(Into::into)
	}

	fn deserialize_unit_struct<V>(self, name: &'static str, visitor: V) -> Result<V::Value>
	where
		V: Visitor<'de>,
	{
		match name {
			"Ignore" => self.record_ignore(),
			"IgnoreAll" => self.record_ignore_all(),
			_ => unimplemented!("Unrecognized deserialization Directive {name:?}"),
		};

		visitor.visit_unit()
	}

	fn deserialize_newtype_struct<V>(self, name: &'static str, visitor: V) -> Result<V::Value>
	where
		V: Visitor<'de>,
	{
		match name {
			"$serde_json::private::RawValue" => visitor.visit_map(self),
			_ => visitor.visit_newtype_struct(self),
		}
	}

	fn deserialize_enum<V>(
		self, _name: &'static str, _variants: &'static [&'static str], _visitor: V,
	) -> Result<V::Value>
	where
		V: Visitor<'de>,
	{
		unimplemented!("deserialize Enum not implemented")
	}

	fn deserialize_option<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		unimplemented!("deserialize Option not implemented")
	}

	fn deserialize_bool<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		unimplemented!("deserialize bool not implemented")
	}

	fn deserialize_i8<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		unimplemented!("deserialize i8 not implemented")
	}

	fn deserialize_i16<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		unimplemented!("deserialize i16 not implemented")
	}

	fn deserialize_i32<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		unimplemented!("deserialize i32 not implemented")
	}

	fn deserialize_i64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		let bytes: [u8; size_of::<i64>()] = self.buf[self.pos..].try_into()?;
		self.inc_pos(size_of::<i64>());
		visitor.visit_i64(i64::from_be_bytes(bytes))
	}

	fn deserialize_u8<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		unimplemented!("deserialize u8 not implemented; try dereferencing the Handle for [u8] access instead")
	}

	fn deserialize_u16<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		unimplemented!("deserialize u16 not implemented")
	}

	fn deserialize_u32<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		unimplemented!("deserialize u32 not implemented")
	}

	fn deserialize_u64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		let bytes: [u8; size_of::<u64>()] = self.buf[self.pos..].try_into()?;
		self.inc_pos(size_of::<u64>());
		visitor.visit_u64(u64::from_be_bytes(bytes))
	}

	fn deserialize_f32<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		unimplemented!("deserialize f32 not implemented")
	}

	fn deserialize_f64<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		unimplemented!("deserialize f64 not implemented")
	}

	fn deserialize_char<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		unimplemented!("deserialize char not implemented")
	}

	fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		let input = self.record_next();
		let out = string::str_from_bytes(input)?;
		visitor.visit_borrowed_str(out)
	}

	fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		let input = self.record_next();
		let out = string::string_from_bytes(input)?;
		visitor.visit_string(out)
	}

	fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		let input = self.record_trail();
		visitor.visit_borrowed_bytes(input)
	}

	fn deserialize_byte_buf<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		unimplemented!("deserialize Byte Buf not implemented")
	}

	fn deserialize_unit<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		unimplemented!("deserialize Unit not implemented")
	}

	// this only used for $serde_json::private::RawValue at this time; see MapAccess
	fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		let input = "$serde_json::private::RawValue";
		visitor.visit_borrowed_str(input)
	}

	fn deserialize_ignored_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
		unimplemented!("deserialize Ignored Any not implemented")
	}

	fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		debug_assert_eq!(
			conduit::debug::type_name::<V>(),
			"serde_json::value::de::<impl serde::de::Deserialize for \
			 serde_json::value::Value>::deserialize::ValueVisitor",
			"deserialize_any: type not expected"
		);

		match self.record_peek_byte() {
			Some(b'{') => self.deserialize_map(visitor),
			_ => self.deserialize_str(visitor),
		}
	}
}

impl<'a, 'de: 'a> de::SeqAccess<'de> for &'a mut Deserializer<'de> {
	type Error = Error;

	fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
	where
		T: DeserializeSeed<'de>,
	{
		if self.pos >= self.buf.len() {
			return Ok(None);
		}

		self.record_start();
		seed.deserialize(&mut **self).map(Some)
	}
}

// this only used for $serde_json::private::RawValue at this time. our db
// schema doesn't have its own map format; we use json for that anyway
impl<'a, 'de: 'a> de::MapAccess<'de> for &'a mut Deserializer<'de> {
	type Error = Error;

	fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
	where
		K: DeserializeSeed<'de>,
	{
		seed.deserialize(&mut **self).map(Some)
	}

	fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
	where
		V: DeserializeSeed<'de>,
	{
		seed.deserialize(&mut **self)
	}
}