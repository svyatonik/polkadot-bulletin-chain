// Copyright Parity Technologies (UK) Ltd.
// This file is part of Cumulus.

// Cumulus is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cumulus is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cumulus.  If not, see <http://www.gnu.org/licenses/>.

//! To be removed file that sends messages to the Polkadot Bulletin chain.
//!
//! Right now we miss the Kawabunga chain, so let's emulate it by sending
//! messages to the Polkadot Bulletin chain.

use crate::xcm_config;
use xcm::latest::prelude::*;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {}

	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(_n: BlockNumberFor<T>) -> frame_support::weights::Weight {
			let send_result = Self::send_dummy_message();
			log::trace!(
				target: "runtime::bridge-messsages-generator",
				"Sent message to People Chain: {:?}",
				send_result,
			);

			// don't bother with weights, because we only use this pallet in test environment
			Weight::zero()
		}
	}

	impl<T: Config> Pallet<T> {
		pub(crate) fn send_dummy_message() -> Result<(XcmHash, MultiAssets), SendError> {
			// see `encoded_test_xcm_message_to_people_chain` test in the Rococo People
			// chain runtime for details
			let encoded_people_chain_call =
				hex_literal::hex!("00040420746573745f6b657928746573745f76616c7565");
			let people_chain_call_weight = Weight::from_parts(20_000_000_000, 8000);

			let destination = xcm_config::KawabungaLocation::get();
			let msg = sp_std::vec![Transact {
				origin_kind: OriginKind::Superuser,
				call: encoded_people_chain_call.to_vec().into(),
				require_weight_at_most: people_chain_call_weight,
			}]
			.into();

			send_xcm::<xcm_config::XcmRouter>(destination, msg)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{ParachainSystem, PolkadotXcm, RuntimeOrigin};

	#[test]
	fn message_to_bulletin_chain_is_sent() {
		sp_io::TestExternalities::new(Default::default()).execute_with(|| {
			PolkadotXcm::force_default_xcm_version(RuntimeOrigin::root(), Some(3)).unwrap();
			ParachainSystem::open_outbound_hrmp_channel_for_benchmarks_or_tests(
				bp_bridge_hub_rococo::BRIDGE_HUB_ROCOCO_PARACHAIN_ID.into(),
			);
			Pallet::<crate::Runtime>::send_dummy_message().unwrap();
		});
	}
}
