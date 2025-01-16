use clap::Subcommand;
use conduwuit::{utils::stream::TryTools, PduCount, Result};
use futures::TryStreamExt;
use ruma::{events::room::message::RoomMessageEventContent, OwnedRoomOrAliasId};

use crate::{admin_command, admin_command_dispatch};

#[admin_command_dispatch]
#[derive(Debug, Subcommand)]
/// Query tables from database
pub(crate) enum RoomTimelineCommand {
	Pdus {
		room_id: OwnedRoomOrAliasId,

		from: Option<String>,

		#[arg(short, long)]
		limit: Option<usize>,
	},

	Last {
		room_id: OwnedRoomOrAliasId,
	},
}

#[admin_command]
pub(super) async fn last(&self, room_id: OwnedRoomOrAliasId) -> Result<RoomMessageEventContent> {
	let room_id = self.services.rooms.alias.resolve(&room_id).await?;

	let result = self
		.services
		.rooms
		.timeline
		.last_timeline_count(None, &room_id)
		.await?;

	Ok(RoomMessageEventContent::notice_markdown(format!("{result:#?}")))
}

#[admin_command]
pub(super) async fn pdus(
	&self,
	room_id: OwnedRoomOrAliasId,
	from: Option<String>,
	limit: Option<usize>,
) -> Result<RoomMessageEventContent> {
	let room_id = self.services.rooms.alias.resolve(&room_id).await?;

	let from: Option<PduCount> = from.as_deref().map(str::parse).transpose()?;

	let result: Vec<_> = self
		.services
		.rooms
		.timeline
		.pdus_rev(None, &room_id, from)
		.try_take(limit.unwrap_or(3))
		.try_collect()
		.await?;

	Ok(RoomMessageEventContent::notice_markdown(format!("{result:#?}")))
}