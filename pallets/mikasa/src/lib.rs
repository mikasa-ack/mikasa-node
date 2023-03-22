#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/reference/frame-pallets/>
pub use pallet::*;

pub(crate) const LOG_TARGET: &str = "runtime::mikasa";

// syntactic sugar for logging.
#[macro_export]
macro_rules! log {
	($level:tt, $patter:expr $(, $values:expr)* $(,)?) => {
		log::$level!(
			target: $crate::LOG_TARGET,
			concat!("[{:?}] 🤖 ", $patter), <frame_system::Pallet<T>>::block_number() $(, $values)*
		)
	};
}

#[frame_support::pallet]
pub mod pallet {

	use frame_support::{
		inherent::Vec, pallet_prelude::*, sp_runtime::AccountId32, traits::Currency,
		weights::Weight,
	};
	use frame_system::pallet_prelude::*;

	/// The default gas limit for a call to a smart contract.
	const DEFAULT_GAS_LIMIT: Weight = Weight::MAX;
	/// Whether or not to run the contract in debug mode.
	const DEBUG: bool = false;
	/// The determinism of the contract.
	const DETERMINISM: pallet_contracts::Determinism = pallet_contracts::Determinism::Deterministic;

	type BalanceOf<T> = <<T as pallet_contracts::Config>::Currency as Currency<
		<T as frame_system::Config>::AccountId,
	>>::Balance;
	/// The maximum length of the async message pool.
	type AsyncMessagePoolMaxLength = ConstU32<10000>;
	/// The length of a selector.
	type SelectorLength = ConstU32<4>;
	#[derive(
		Clone,
		Debug,
		PartialEq,
		Eq,
		codec::Encode,
		codec::Decode,
		scale_info::TypeInfo,
		codec::MaxEncodedLen,
	)]
	#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
	pub struct AsyncMessage {
		/// The account that sent the message.
		pub sender: AccountId32,
		/// The address of the contract to run autonomous call on.
		pub target_contract: AccountId32,
		/// The selector of the function to call.
		pub target_selector: BoundedVec<u8, SelectorLength>,
		/// An optional selector to check if the call should be made.
		pub should_run_selector: Option<BoundedVec<u8, SelectorLength>>,
		/// An optional selector to check if the message should be removed from the pool.
		pub should_kill_selector: Option<BoundedVec<u8, SelectorLength>>,
		/// The maximal amount of gas to use for the call.
		pub gas_limit: Weight,
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_contracts::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type Currency: Currency<Self::AccountId>;
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		/// The block is being finalized.
		fn on_finalize(_n: T::BlockNumber) {}

		/// The block is being initialized. Implement to have something happen.
		fn on_initialize(_: T::BlockNumber) -> Weight {
			Self::process_async_message_pool();
			Weight::zero()
		}

		/// Perform a module upgrade.
		fn on_runtime_upgrade() -> Weight {
			Weight::zero()
		}

		/// Run offchain tasks.
		fn offchain_worker(n: T::BlockNumber) {
			log!(trace, "Running offchain worker at block {:?}.", n,)
		}
	}

	#[pallet::storage]
	#[pallet::getter(fn async_message_pool)]
	pub(super) type AsyncMessagePool<T: Config> =
		StorageValue<_, BoundedVec<AsyncMessage, AsyncMessagePoolMaxLength>, OptionQuery>;

	// Pallets use events to inform users when important changes are made.
	// https://docs.substrate.io/main-docs/build/events-errors/
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event generated when an autonomous call is made to a smart contract.
		AutonomousSmartContractCall(T::AccountId),
		/// Event generated when a call to a smart contract is made through an extrinsic.
		SmartContractCallThroughExtrinsic(T::AccountId),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(0)]
		pub fn call_smart_contract(
			origin: OriginFor<T>,
			destination_address: T::AccountId,
			mut selector: Vec<u8>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let value: BalanceOf<T> = Default::default();
			let mut data = Vec::new();
			data.append(&mut selector);

			// Do the actual call to the smart contract function
			pallet_contracts::Pallet::<T>::bare_call(
				who,
				destination_address.clone(),
				value,
				DEFAULT_GAS_LIMIT,
				Self::storage_deposit_limit(),
				data,
				DEBUG,
				DETERMINISM,
			)
			.result?;
			Self::deposit_event(Event::SmartContractCallThroughExtrinsic(destination_address));
			Ok(())
		}

		/// Register an autonomous call to a smart contract.
		/// The call will be executed started from the next block.
		/// # Arguments
		/// * `target_contract` - The address of the smart contract to call.
		/// * `selector` - The selector of the function to call.
		#[pallet::call_index(1)]
		#[pallet::weight(0)]
		pub fn register_async_message(
			origin: OriginFor<T>,
			target_contract: T::AccountId,
			selector: Vec<u8>,
			should_run_selector: Option<Vec<u8>>,
			should_kill_selector: Option<Vec<u8>>,
			gas_limit: Option<Weight>,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			let sender = Self::account_id_to_account_id32(sender);
			let target_contract = Self::account_id_to_account_id32(target_contract);
			let target_selector = selector.try_into().unwrap();
			let should_run_selector = should_run_selector.map(|s| s.try_into().unwrap());
			let should_kill_selector = should_kill_selector.map(|s| s.try_into().unwrap());
			let async_message = AsyncMessage {
				sender,
				target_contract,
				target_selector,
				should_run_selector,
				should_kill_selector,
				gas_limit: gas_limit.unwrap_or(DEFAULT_GAS_LIMIT),
			};
			AsyncMessagePool::<T>::try_append(async_message).unwrap();
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Process the async message pool.
		/// This function is called at the beginning of each block.
		/// It iterates over the async message pool and executes the autonomous calls.
		fn process_async_message_pool() {
			// Read the async message pool
			let async_message_pool = AsyncMessagePool::<T>::get().unwrap_or_default();
			// Iterate over the async message pool
			for (index, async_message) in async_message_pool.iter().enumerate() {
				Self::process_async_message(index, async_message.clone());
			}
		}

		/// Process an async message.
		/// # Arguments
		/// * `async_message` - The async message to process.
		fn process_async_message(index: usize, async_message: AsyncMessage) {
			let target_contract = Self::account_id32_to_account_id(async_message.target_contract);
			log!(info, "Triggering autonomous call on contract {:?}.", target_contract.clone());

			let sender = Self::account_id32_to_account_id(async_message.sender);
			let value: BalanceOf<T> = Default::default();
			let mut selector = async_message.target_selector.into_inner();
			let mut data = Vec::new();

			data.append(&mut selector);

			// Check if the call should be run
			if let Some(should_run_selector) = async_message.should_run_selector {
				log!(info, "Checking if autonomous call should be run.");
				let should_run = Self::call_and_get_bool(
					sender.clone(),
					target_contract.clone(),
					should_run_selector.into_inner(),
				);
				if !should_run {
					log!(info, "Autonomous call should not be run. Skipping.");
					return
				}
				log!(info, "Autonomous call should be run. Executing.");
			}

			// Call the contract
			let _result = pallet_contracts::Pallet::<T>::bare_call(
				sender.clone(),
				target_contract.clone(),
				value,
				DEFAULT_GAS_LIMIT,
				Self::storage_deposit_limit(),
				data,
				DEBUG,
				DETERMINISM,
			)
			.result
			.unwrap();
			// Check if the call should be killed
			if let Some(should_kill_selector) = async_message.should_kill_selector {
				let should_kill = Self::call_and_get_bool(
					sender.clone(),
					target_contract.clone(),
					should_kill_selector.into_inner(),
				);
				if should_kill {
					log!(info, "Autonomous call should be killed. Removing from pool.");
					let mut async_message_pool = AsyncMessagePool::<T>::get().unwrap_or_default();
					async_message_pool.remove(index);
					AsyncMessagePool::<T>::set(Some(async_message_pool));
				}
			}
			// Emit an event
			Self::deposit_event(Event::AutonomousSmartContractCall(target_contract));
		}

		/// Call a target contract function and return the result as a boolean.
		/// Assumes the selector actually matches a function that return a boolean.
		/// # Arguments
		/// - `who`: the account of the sender
		/// - `destination_address`: the target address of the smart contract
		/// - `selector`: the selector of the function to call.
		fn call_and_get_bool(
			who: T::AccountId,
			destination_address: T::AccountId,
			mut selector: Vec<u8>,
		) -> bool {
			let value: BalanceOf<T> = Default::default();
			let mut data = Vec::new();
			data.append(&mut selector);

			// Do the actual call to the smart contract function
			let result = pallet_contracts::Pallet::<T>::bare_call(
				who,
				destination_address.clone(),
				value,
				DEFAULT_GAS_LIMIT,
				Self::storage_deposit_limit(),
				data,
				DEBUG,
				DETERMINISM,
			)
			.result
			.unwrap();

			let encoded_data = result.data;

			// For some reason the decoding as boolean does not work
			// Let's just take the second byte and convert it to a boolean
			//bool::decode(&mut &encoded_data[..]).unwrap()
			encoded_data[1] != 0
		}

		/// Convert AccountId32 to T::AccountId
		/// # Arguments
		/// * `account_id` - AccountId32
		/// # Returns
		/// * `T::AccountId`
		fn account_id32_to_account_id(account_id: AccountId32) -> T::AccountId {
			let mut to32 = AccountId32::as_ref(&account_id);
			let to_address: T::AccountId = T::AccountId::decode(&mut to32).unwrap();
			to_address
		}

		/// Convert T::AccountId to AccountId32
		/// # Arguments
		/// * `account_id` - T::AccountId
		/// # Returns
		/// * `AccountId32`
		fn account_id_to_account_id32(account_id: T::AccountId) -> AccountId32 {
			let a = account_id.encode();
			let mut a32 = [0u8; 32];
			a32.copy_from_slice(&a);
			AccountId32::from(a32)
		}

		fn storage_deposit_limit() -> Option<BalanceOf<T>> {
			None
		}
	}
}
