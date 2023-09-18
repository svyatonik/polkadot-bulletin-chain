//! With Polkadot Bridge Hub bridge configuration.

use crate::{AccountId, Runtime, RuntimeEvent, RuntimeOrigin};

use bp_messages::{LaneId, MessageNonce};
use bp_parachains::SingleParaStoredHeaderDataBuilder;
use bp_runtime::{ChainId, UnderlyingChainProvider};
use bridge_runtime_common::{
	messages::{
		source::{
			FromThisChainMaximalOutboundPayloadSize, FromThisChainMessageVerifier,
			TargetHeaderChainAdapter,
		},
		target::SourceHeaderChainAdapter,
		BridgedChainWithMessages, MessageBridge, ThisChainWithMessages,
	},
	messages_xcm_extension::{
		SenderAndLane, XcmAsPlainPayload, XcmBlobHauler, XcmBlobHaulerAdapter, XcmBlobMessageDispatch,
	},
};
use frame_support::{parameter_types, RuntimeDebug};
use sp_runtime::transaction_validity::{InvalidTransaction, TransactionValidity};
use sp_std::vec::Vec;
use xcm::prelude::*;
use xcm_builder::HaulBlobExporter;

/// Lane that we are using to send and receive messages.
pub const XCM_LANE: LaneId = LaneId([0, 0, 0, 0]);

parameter_types! {
	/// A set of message relayers, who are able to submit message delivery transactions
	/// and physically deliver messages on this chain.
	///
	/// It can be changed by the governance later.
	pub storage WhitelistedRelayers: Vec<AccountId> = {
		crate::Sudo::key().map(|sudo_key| sp_std::vec![sudo_key]).unwrap_or_default()
	};

	/// A number of Polkadot mandatory headers that are accepted for free at every
	/// **this chain** block.
	pub const MaxFreePolkadotHeadersPerBlock: u32 = 4;
	/// A number of Polkadot header digests that we keep in the storage.
	pub const PolkadotHeadersToKeep: u32 = 1024;
	/// A name of parachains pallet at Pokadot.
	pub const AtPolkadotParasPalletName: &'static str = bp_polkadot::PARAS_PALLET_NAME;

	/// The Polkadot Chain network ID.
	pub const PolkadotNetwork: NetworkId = Polkadot;
	/// Chain identifier of Polkadot Bridge Hub.
	pub const BridgeHubPolkadotChainId: ChainId = bp_runtime::BRIDGE_HUB_POLKADOT_CHAIN_ID;
	/// A number of Polkadot Bridge Hub head digests that we keep in the storage.
	pub const BridgeHubPolkadotHeadsToKeep: u32 = 1024;
	/// A maximal size of Polkadot Bridge Hub head digest.
	pub const MaxPolkadotBrdgeHubHeadSize: u32 = bp_polkadot::MAX_NESTED_PARACHAIN_HEAD_DATA_SIZE;

	/// All active outbound lanes.
	pub const ActiveOutboundLanes: &'static [LaneId] = &[XCM_LANE];
	/// Maximal number of unrewarded relayer entries.
	pub const MaxUnrewardedRelayerEntriesAtInboundLane: MessageNonce =
		bp_bridge_hub_polkadot::MAX_UNREWARDED_RELAYERS_IN_CONFIRMATION_TX;
	/// Maximal number of unconfirmed messages.
	pub const MaxUnconfirmedMessagesAtInboundLane: MessageNonce =
		bp_bridge_hub_polkadot::MAX_UNCONFIRMED_MESSAGES_IN_CONFIRMATION_TX;

	/// Sending chain location and lane used to communicate with Polkadot Bulletin chain.
	pub FromPolkadotBulletinToBridgeHubPolkadotRoute: SenderAndLane = SenderAndLane::new(
		Here.into(),
		XCM_LANE,
	);

	/// XCM message that is never sent to anyone.
	pub NeverSentMessage: Option<Xcm<()>> = None;
}

/// An instance of `pallet_bridge_grandpa` used to bridge with Polkadot.
pub type WithPolkadotBridgeGrandpaInstance = ();
/// An instance of `pallet_bridge_parachains` used to bridge with Polkadot.
pub type WithPolkadotBridgeParachainsInstance = ();
/// An instance of `pallet_bridge_messages` used to bridge with Polkadot Bridge Hub.
pub type WithBridgeHubPolkadotMessagesInstance = ();

impl pallet_bridge_grandpa::Config<WithPolkadotBridgeGrandpaInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = (); // TODO

	type BridgedChain = bp_polkadot::Polkadot;
	type MaxFreeMandatoryHeadersPerBlock = MaxFreePolkadotHeadersPerBlock;
	type HeadersToKeep = PolkadotHeadersToKeep;
}

impl pallet_bridge_parachains::Config<WithPolkadotBridgeParachainsInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = (); // TODO

	type BridgesGrandpaPalletInstance = WithPolkadotBridgeGrandpaInstance;
	type ParasPalletName = AtPolkadotParasPalletName;
	type ParaStoredHeaderDataBuilder =
		SingleParaStoredHeaderDataBuilder<bp_bridge_hub_polkadot::BridgeHubPolkadot>;
	type HeadsToKeep = BridgeHubPolkadotHeadsToKeep;
	type MaxParaHeadDataSize = MaxPolkadotBrdgeHubHeadSize;
}

impl pallet_bridge_messages::Config<WithBridgeHubPolkadotMessagesInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = (); // TODO

	type BridgedChainId = BridgeHubPolkadotChainId;
	type ActiveOutboundLanes = ActiveOutboundLanes;
	type MaxUnrewardedRelayerEntriesAtInboundLane = MaxUnrewardedRelayerEntriesAtInboundLane;
	type MaxUnconfirmedMessagesAtInboundLane = MaxUnconfirmedMessagesAtInboundLane;

	type MaximalOutboundPayloadSize =
		FromThisChainMaximalOutboundPayloadSize<WithBridgeHubPolkadotMessageBridge>;
	type OutboundPayload = XcmAsPlainPayload;

	type InboundPayload = XcmAsPlainPayload;
	type InboundRelayer = AccountId;
	type DeliveryPayments = ();

	type TargetHeaderChain = TargetHeaderChainAdapter<WithBridgeHubPolkadotMessageBridge>;
	type LaneMessageVerifier = FromThisChainMessageVerifier<WithBridgeHubPolkadotMessageBridge>;
	type DeliveryConfirmationPayments = ();

	type SourceHeaderChain = SourceHeaderChainAdapter<WithBridgeHubPolkadotMessageBridge>;
	type MessageDispatch = XcmBlobMessageDispatch<
		FromBridgeHubPolkadotBlobDispatcher,
		Self::WeightInfo,
		(),
	>;
	type OnMessagesDelivered = ();
}

/// Message bridge with Polkadot Bridge Hub.
pub struct WithBridgeHubPolkadotMessageBridge;

impl MessageBridge for WithBridgeHubPolkadotMessageBridge {
	const BRIDGED_MESSAGES_PALLET_NAME: &'static str =
		bp_polkadot_bulletin::WITH_POLKADOT_BULLETIN_MESSAGES_PALLET_NAME;
	type ThisChain = PolkadotBulletinChain;
	type BridgedChain = BridgeHubPolkadot;
	type BridgedHeaderChain = pallet_bridge_parachains::ParachainHeaders<
		Runtime,
		WithPolkadotBridgeParachainsInstance,
		bp_bridge_hub_polkadot::BridgeHubPolkadot,
	>;
}

/// BridgeHubPolkadot chain from message lane point of view.
#[derive(RuntimeDebug, Clone, Copy)]
pub struct BridgeHubPolkadot;

impl UnderlyingChainProvider for BridgeHubPolkadot {
	type Chain = bp_bridge_hub_polkadot::BridgeHubPolkadot;
}

impl BridgedChainWithMessages for BridgeHubPolkadot {}

/// BridgeHubRococo chain from message lane point of view.
#[derive(RuntimeDebug, Clone, Copy)]
pub struct PolkadotBulletinChain;

impl UnderlyingChainProvider for PolkadotBulletinChain {
	type Chain = bp_polkadot_bulletin::PolkadotBulletin;
}

impl ThisChainWithMessages for PolkadotBulletinChain {
	type RuntimeOrigin = RuntimeOrigin;
}

/// Dispatches received XCM messages from the Polkadot Bridge Hub.
pub type FromBridgeHubPolkadotBlobDispatcher = crate::xcm_config::ImmediateXcmDispatcher;

/// Export XCM messages to be relayed to the Polkadot Bridge Hub chain.
pub type ToBridgeHubPolkadotHaulBlobExporter =
	HaulBlobExporter<XcmBlobHaulerAdapter<ToBridgeHubPolkadotXcmBlobHauler>, PolkadotNetwork, ()>;
pub struct ToBridgeHubPolkadotXcmBlobHauler;
impl XcmBlobHauler for ToBridgeHubPolkadotXcmBlobHauler {
	type Runtime = Runtime;
	type MessagesInstance = WithBridgeHubPolkadotMessagesInstance;
	type SenderAndLane = FromPolkadotBulletinToBridgeHubPolkadotRoute;

	type ToSourceChainSender = ();
	type CongestedMessage = NeverSentMessage;
	type UncongestedMessage = NeverSentMessage;
}

/// Ensure that the account provided is the whitelisted relayer account.
pub fn ensure_whitelisted_relayer(who: &AccountId) -> TransactionValidity {
	if !WhitelistedRelayers::get().contains(who) {
		return Err(InvalidTransaction::BadSigner.into())
	}

	Ok(Default::default())
}

#[cfg(test)]
mod tests {
	use crate::RuntimeCall;
	use super::*;
	use codec::Encode;

	#[test]
	fn may_change_whitelisted_relayers_set_using_sudo() {
		// TODO
	}

	#[test]
	fn whitelisted_relayer_may_submit_polkadot_headers() {
		// TODO
	}

	#[test]
	fn regular_account_can_not_submit_polkadot_headers() {
		// TODO
	}

	#[test]
	fn whitelisted_relayer_may_submit_polkadot_bridge_hub_headers() {
		// TODO
	}

	#[test]
	fn regular_account_can_not_submit_polkadot_bridge_hub_headers() {
		// TODO
	}

	#[test]
	fn whitelisted_relayer_may_submit_messages_and_confirmations_from_polkadot_bridge_hub() {
		// TODO
	}

	#[test]
	fn regular_account_can_not_submit_messages_and_confirmations_from_polkadot_bridge_hub() {
		// TODO
	}

	#[test]
	fn encoded_test_xcm_message_to_bulletin_chain() {
		let universal_dest: VersionedInteriorMultiLocation
			= X1(GlobalConsensus(crate::xcm_config::ThisNetwork::get())).into();
		let xcm: Xcm<RuntimeCall> = vec![
			Transact {
				origin_kind: OriginKind::Superuser,
				call: RuntimeCall::System(frame_system::Call::remark { remark: vec![42] }).encode().into(),
				require_weight_at_most: Weight::from_parts(20_000_000_000, 8000),
			}
		].into();
		let xcm = VersionedXcm::<RuntimeCall>::V3(xcm);
		// XCM BridgeMessage - a pair of `VersionedInteriorMultiLocation` and `VersionedXcm<()>`
		let encoded_xcm_message = (universal_dest, xcm).encode();
		println!("{}", hex::encode(&encoded_xcm_message));
	}
}
