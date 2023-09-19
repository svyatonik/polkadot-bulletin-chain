// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

//! XCM configuration for Polkadot Bulletin chain.

use crate::{
	bridge_config::ToBridgeHubPolkadotHaulBlobExporter, AllPalletsWithSystem, RuntimeCall,
	RuntimeOrigin,
};

use bridge_runtime_common::messages_xcm_extension::XcmAsPlainPayload;
use codec::{Decode, Encode};
use frame_support::{
	ensure, match_types, parameter_types,
	traits::{Contains, Nothing, ProcessMessageError},
	weights::Weight,
};
use sp_core::ConstU32;
use sp_io::hashing::blake2_256;
use xcm::{latest::prelude::*, DoubleEncoded, VersionedInteriorMultiLocation, VersionedXcm};
use xcm_builder::{
	CreateMatcher, DispatchBlob, DispatchBlobError, FixedWeightBounds, MatchXcm,
	TrailingSetTopicAsId, UnpaidLocalExporter,
};
use xcm_executor::{
	traits::{ConvertOrigin, ShouldExecute, WeightTrader, WithOriginFilter},
	Assets, XcmExecutor,
};

const KAWABUNGA_PARACHAIN_ID: u32 = 42;

parameter_types! {
	/// The Polkadot Bulletin Chain network ID.
	pub const ThisNetwork: NetworkId = NetworkId::ByGenesis([42u8; 32]); // TODO
	/// Our location in the universe of consensus systems.
	pub const UniversalLocation: InteriorMultiLocation = X1(GlobalConsensus(ThisNetwork::get()));

	/// Location of the Kawabunga chain, relative to this runtime.
	pub KawabungaLocation: MultiLocation = MultiLocation::new(1, X2(
		GlobalConsensus(Polkadot),
		Parachain(KAWABUNGA_PARACHAIN_ID),
	));

	/// The amount of weight an XCM operation takes. This is a safe overestimate.
	pub const BaseXcmWeight: Weight = Weight::from_parts(1_000_000_000, 0);
	/// Maximum number of instructions in a single XCM fragment. A sanity check against weight
	/// calculations getting too crazy.
	pub const MaxInstructions: u32 = 100;
}

match_types! {
	// Only contains Kawabunga parachain location.
	pub type OnlyKawabungaLocation: impl Contains<MultiLocation> = {
		MultiLocation { parents: 1, interior: X2(GlobalConsensus(Polkadot), Parachain(KAWABUNGA_PARACHAIN_ID)) }
	};

	// Only passes calls that may be called using XCM transact through the bridge.
	pub type AllowedXcmTransactCalls: impl Contains<RuntimeCall> = {
		_ // TODO
	};
}

/// Kawabunga location converter to local root.
pub struct KawabungaParachainAsRoot;

impl ConvertOrigin<RuntimeOrigin> for KawabungaParachainAsRoot {
	fn convert_origin(
		origin: impl Into<MultiLocation>,
		kind: OriginKind,
	) -> Result<RuntimeOrigin, MultiLocation> {
		let origin = origin.into();
		log::trace!(
			target: "xcm::origin_conversion",
			"KawabungaParachainAsRoot origin: {:?}, kind: {:?}",
			origin, kind,
		);
		match (kind, origin) {
			(
				OriginKind::Superuser,
				MultiLocation {
					parents: 1,
					interior: X2(GlobalConsensus(remote_network), Parachain(remote_parachain)),
				},
			) if remote_network == Polkadot && remote_parachain == KAWABUNGA_PARACHAIN_ID =>
				Ok(RuntimeOrigin::root()),
			(_, origin) => Err(origin),
		}
	}
}

/// Weight trader that does nothing.
pub struct NoopTrader;

impl WeightTrader for NoopTrader {
	fn new() -> Self {
		NoopTrader
	}

	fn buy_weight(&mut self, _weight: Weight, _payment: Assets) -> Result<Assets, XcmError> {
		Ok(Assets::new())
	}

	fn refund_weight(&mut self, _weight: Weight) -> Option<MultiAsset> {
		None
	}
}

/// Allows execution from `origin` if it is contained in `AllowedOrigin`
/// and if it is just a straight `Transact` which contains `AllowedCall`.
///
/// That's a 1:1 copy of corresponding Cumulus structire.
pub struct AllowUnpaidTransactsFrom<RuntimeCall, AllowedCall, AllowedOrigin>(
	sp_std::marker::PhantomData<(RuntimeCall, AllowedCall, AllowedOrigin)>,
);
impl<
		RuntimeCall: Decode,
		AllowedCall: Contains<RuntimeCall>,
		AllowedOrigin: Contains<MultiLocation>,
	> ShouldExecute for AllowUnpaidTransactsFrom<RuntimeCall, AllowedCall, AllowedOrigin>
{
	fn should_execute<Call>(
		origin: &MultiLocation,
		instructions: &mut [Instruction<Call>],
		max_weight: Weight,
		_properties: &mut xcm_executor::traits::Properties,
	) -> Result<(), ProcessMessageError> {
		log::trace!(
			target: "xcm::barriers",
			"AllowUnpaidTransactFrom origin: {:?}, instructions: {:?}, max_weight: {:?}, properties: {:?}",
			origin, instructions, max_weight, _properties,
		);

		// we only allow from configured origins
		ensure!(AllowedOrigin::contains(origin), ProcessMessageError::Unsupported);

		// we expect an XCM program with single `Transact` call
		instructions
			.matcher()
			.assert_remaining_insts(1)?
			.match_next_inst(|inst| match inst {
				Transact { origin_kind: OriginKind::Superuser, call: encoded_call, .. } => {
					// this is a hack - don't know if there's a way to do that properly
					// or else we can simply allow all calls
					let mut decoded_call = DoubleEncoded::<RuntimeCall>::from(encoded_call.clone());
					ensure!(
						AllowedCall::contains(
							decoded_call
								.ensure_decoded()
								.map_err(|_| ProcessMessageError::BadFormat)?
						),
						ProcessMessageError::BadFormat,
					);

					Ok(())
				},
				_ => Err(ProcessMessageError::BadFormat),
			})?;

		Ok(())
	}
}

/// The means that we convert an XCM origin `MultiLocation` into the runtime's `Origin` type for
/// local dispatch. This is a conversion function from an `OriginKind` type along with the
/// `MultiLocation` value and returns an `Origin` value or an error.
type LocalOriginConverter = (
	// Currently we only accept XCM messages from Kawabunga and the origin for such messages
	// is local root.
	KawabungaParachainAsRoot,
);

/// Only bridged destination is supported.
pub type XcmRouter = UnpaidLocalExporter<ToBridgeHubPolkadotHaulBlobExporter, UniversalLocation>;

/// The barriers one of which must be passed for an XCM message to be executed.
pub type Barrier = TrailingSetTopicAsId<
	// We only allow unpaid execution from the Kawabunga parachain.
	AllowUnpaidTransactsFrom<RuntimeCall, AllowedXcmTransactCalls, OnlyKawabungaLocation>,
>;

/// XCM executor configuration.
pub struct XcmConfig;

impl xcm_executor::Config for XcmConfig {
	type RuntimeCall = RuntimeCall;
	type XcmSender = XcmRouter;
	type AssetTransactor = ();
	type OriginConverter = LocalOriginConverter;
	type IsReserve = ();
	type IsTeleporter = ();
	type UniversalLocation = UniversalLocation;
	type Barrier = Barrier;
	type Weigher = FixedWeightBounds<BaseXcmWeight, RuntimeCall, MaxInstructions>; // TODO
	type Trader = NoopTrader;
	type ResponseHandler = ();
	type AssetTrap = ();
	type AssetLocker = ();
	type AssetExchanger = ();
	type AssetClaims = ();
	type SubscriptionService = ();
	type PalletInstancesInfo = AllPalletsWithSystem;
	type MaxAssetsIntoHolding = ConstU32<0>;
	type FeeManager = ();
	type MessageExporter = ToBridgeHubPolkadotHaulBlobExporter;
	type UniversalAliases = Nothing;
	type CallDispatcher = WithOriginFilter<AllowedXcmTransactCalls>;
	type SafeCallFilter = AllowedXcmTransactCalls;
	type Aliasers = Nothing;
}

// TODO: below shall be either static (benchmarked) weight, or simply insert message to
// the queue for later dispatch. This version is for tests only

/// XCM blob dispatcher that executes XCM message at this chain.
///
/// That's a copy of `xcm_builder::BridgeBlobDispatcher` struct. The only difference is
/// that instead of sending XCM further, it dispatches the message immediately.
pub struct ImmediateXcmDispatcher;

impl DispatchBlob for ImmediateXcmDispatcher {
	fn dispatch_blob(blob: XcmAsPlainPayload) -> Result<(), DispatchBlobError> {
		let our_universal = UniversalLocation::get();
		let our_global =
			our_universal.global_consensus().map_err(|()| DispatchBlobError::Unbridgable)?;
		// internally it is the encoded `BridgeMessage`, but it is a private struct, so we
		// are simply decoding pair here
		let (universal_dest, message): (VersionedInteriorMultiLocation, VersionedXcm<RuntimeCall>) =
			Decode::decode(
				// TODO: decode_all_with_depth_limit?
				&mut &blob[..],
			)
			.map_err(|_| DispatchBlobError::InvalidEncoding)?;
		let universal_dest: InteriorMultiLocation = universal_dest
			.try_into()
			.map_err(|_| DispatchBlobError::UnsupportedLocationVersion)?;
		// `universal_dest` is the desired destination within the universe: first we need to check
		// we're in the right global consensus.
		let intended_global = universal_dest
			.global_consensus()
			.map_err(|()| DispatchBlobError::NonUniversalDestination)?;
		ensure!(intended_global == our_global, DispatchBlobError::WrongGlobal);
		let message: Xcm<RuntimeCall> =
			message.try_into().map_err(|_| DispatchBlobError::UnsupportedXcmVersion)?;

		// TODO: insert pallet discriminator?

		log::trace!(
			target: "runtime::xcm",
			"Going to dispatch XCM message from {:?}: {:?}",
			KawabungaLocation::get(),
			message,
		);

		// execute the XCM program
		let message_hash = message.using_encoded(blake2_256);
		XcmExecutor::<XcmConfig>::execute_xcm(
			KawabungaLocation::get(),
			message,
			message_hash,
			Weight::MAX, // TODO
		)
		.ensure_complete()
		.map_err(|e| {
			log::trace!(
				target: "runtime::xcm",
				"XCM message from {:?} was dispatched with an error: {:?}",
				KawabungaLocation::get(),
				e,
			);

			DispatchBlobError::RoutingError
		})?; // TODO: this is bad

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{
		bridge_config::{tests::run_test, WithBridgeHubPolkadotMessagesInstance, XCM_LANE},
		BridgePolkadotMessages, Runtime,
	};
	use bp_messages::{
		target_chain::{DispatchMessage, DispatchMessageData, MessageDispatch},
		MessageKey,
	};
	use pallet_bridge_messages::Config as MessagesConfig;

	type Dispatcher =
		<Runtime as MessagesConfig<WithBridgeHubPolkadotMessagesInstance>>::MessageDispatch;

	fn test_storage_key() -> Vec<u8> {
		(*b"test_key").to_vec()
	}

	fn test_storage_value() -> Vec<u8> {
		(*b"test_value").to_vec()
	}

	fn encoded_xcm_message_from_bridge_hub_polkadot() -> Vec<u8> {
		let universal_dest: VersionedInteriorMultiLocation =
			X1(GlobalConsensus(crate::xcm_config::ThisNetwork::get())).into();
		let xcm: Xcm<RuntimeCall> = vec![Transact {
			origin_kind: OriginKind::Superuser,
			call: RuntimeCall::System(frame_system::Call::set_storage {
				items: vec![(test_storage_key(), test_storage_value())],
			})
			.encode()
			.into(),
			require_weight_at_most: Weight::from_parts(20_000_000_000, 8000),
		}]
		.into();
		let xcm = VersionedXcm::<RuntimeCall>::V3(xcm);
		// XCM BridgeMessage - a pair of `VersionedInteriorMultiLocation` and `VersionedXcm<()>`
		(universal_dest, xcm).encode()
	}

	#[test]
	fn messages_from_bridge_hub_polkadot_are_dispatched() {
		run_test(|| {
			assert_eq!(frame_support::storage::unhashed::get_raw(&test_storage_key()), None);
			Dispatcher::dispatch(DispatchMessage {
				key: MessageKey { lane_id: XCM_LANE, nonce: 1 },
				data: DispatchMessageData {
					payload: Ok(encoded_xcm_message_from_bridge_hub_polkadot()),
				},
			});
			assert_eq!(
				frame_support::storage::unhashed::get_raw(&test_storage_key()),
				Some(test_storage_value()),
			);
		});
	}

	#[test]
	fn messages_to_bridge_hub_polkadot_are_sent() {
		run_test(|| {
			assert_eq!(
				BridgePolkadotMessages::outbound_lane_data(XCM_LANE).latest_generated_nonce,
				0
			);
			send_xcm::<XcmRouter>(KawabungaLocation::get(), vec![ClearOrigin].into())
				.expect("message is sent");
			assert_ne!(
				BridgePolkadotMessages::outbound_lane_data(XCM_LANE).latest_generated_nonce,
				0
			);
		})
	}

	#[test]
	fn encoded_test_xcm_message_to_bulletin_chain() {
		// this "test" is currently used to encode dummy message for Polkadot BH -> Bulletin
		// bridge. Once we have real sending chain (Kawabunga), it could be removed
		println!("{}", hex::encode(&encoded_xcm_message_from_bridge_hub_polkadot()));
	}
}
