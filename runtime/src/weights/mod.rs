//! Expose the auto generated weight files.

use frame_support::weights::Weight;

pub mod bridge_polkadot_grandpa;
pub mod bridge_polkadot_messages;
pub mod bridge_polkadot_parachains;

impl pallet_bridge_parachains::WeightInfoExt
	for bridge_polkadot_parachains::WeightInfo<crate::Runtime>
{
	fn expected_extra_storage_proof_size() -> u32 {
		bp_bridge_hub_polkadot::EXTRA_STORAGE_PROOF_SIZE
	}
}

impl pallet_bridge_messages::WeightInfoExt
	for bridge_polkadot_messages::WeightInfo<crate::Runtime>
{
	fn expected_extra_storage_proof_size() -> u32 {
		bp_bridge_hub_polkadot::EXTRA_STORAGE_PROOF_SIZE
	}

	fn receive_messages_proof_overhead_from_runtime() -> Weight {
		Weight::zero()
	}

	fn receive_messages_delivery_proof_overhead_from_runtime() -> Weight {
		Weight::zero()
	}
}
