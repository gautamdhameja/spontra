use crate::Config;
use core::marker::PhantomData;
use frame_support::traits::{CallMetadata, GetCallMetadata};
use pallet_transaction_payment::{Config as TxPyConfig, OnChargeTransaction};
use sp_std::prelude::*;

use sp_runtime::{
	traits::{DispatchInfoOf, PostDispatchInfoOf, Zero},
	transaction_validity::InvalidTransaction,
};

use frame_support::{
	traits::{Currency, ExistenceRequirement, Imbalance, OnUnbalanced, WithdrawReasons},
	unsigned::TransactionValidityError,
};

pub struct PayerFinder<T>(PhantomData<T>);

impl<T: Config> PayerFinder<T>
where
	<T as frame_system::Config>::RuntimeCall: GetCallMetadata,
{
	pub(crate) fn get_payer_account() -> Option<T::AccountId> {
		<crate::PayerKey<T>>::get()
	}

	pub(crate) fn is_call_sponsored(call: &T::RuntimeCall) -> bool {
		let CallMetadata { pallet_name, function_name } = call.get_call_metadata();
		Self::is_sponsored_unbound(pallet_name.into(), function_name.into())
	}

	fn is_sponsored_unbound(pallet: Vec<u8>, call: Vec<u8>) -> bool {
		let pallet = crate::PalletNameOf::<T>::try_from(pallet);
		let call = crate::PalletCallNameOf::<T>::try_from(call);

		match (pallet, call) {
			(Ok(pallet), Ok(call)) => crate::SponsoredCalls::<T>::contains_key(&(pallet, call)),
			_ => true,
		}
	}
}

type NegativeImbalanceOf<C, T> =
	<C as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

pub struct SponsoredCurrencyAdapter<C, OU>(PhantomData<(C, OU)>);

/// Default implementation for a Currency and an OnUnbalanced handler.
///
/// The unbalance handler is given 2 unbalanceds in [`OnUnbalanced::on_unbalanceds`]: `fee` and
/// then `tip`.
#[allow(deprecated)]
impl<T, C, OU> OnChargeTransaction<T> for SponsoredCurrencyAdapter<C, OU>
where
	T: TxPyConfig,
	T: Config,
	C: Currency<<T as frame_system::Config>::AccountId>,
	C::PositiveImbalance: Imbalance<
		<C as Currency<<T as frame_system::Config>::AccountId>>::Balance,
		Opposite = C::NegativeImbalance,
	>,
	C::NegativeImbalance: Imbalance<
		<C as Currency<<T as frame_system::Config>::AccountId>>::Balance,
		Opposite = C::PositiveImbalance,
	>,
	OU: OnUnbalanced<NegativeImbalanceOf<C, T>>,
	<T as frame_system::Config>::RuntimeCall: GetCallMetadata,
{
	type LiquidityInfo = Option<NegativeImbalanceOf<C, T>>;
	type Balance = <C as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	/// Withdraw the predicted fee from the transaction origin.
	///
	/// Note: The `fee` already includes the `tip`.
	fn withdraw_fee(
		who: &T::AccountId,
		call: &T::RuntimeCall,
		_info: &DispatchInfoOf<T::RuntimeCall>,
		fee: Self::Balance,
		tip: Self::Balance,
	) -> Result<Self::LiquidityInfo, TransactionValidityError> {
		if fee.is_zero() {
			return Ok(None)
		}

		let withdraw_reason = if tip.is_zero() {
			WithdrawReasons::TRANSACTION_PAYMENT
		} else {
			WithdrawReasons::TRANSACTION_PAYMENT | WithdrawReasons::TIP
		};

		if PayerFinder::<T>::is_call_sponsored(call) {
			if let Some(payer) = PayerFinder::<T>::get_payer_account() {
				match C::withdraw(&payer, fee, withdraw_reason, ExistenceRequirement::KeepAlive) {
					Ok(imbalance) => Ok(Some(imbalance)),
					Err(_) => Err(InvalidTransaction::Payment.into()),
				}
			} else {
				Err(InvalidTransaction::Payment.into())
			}
		} else {
			// Fall back to sender as payer.
			match C::withdraw(&who, fee, withdraw_reason, ExistenceRequirement::KeepAlive) {
				Ok(imbalance) => Ok(Some(imbalance)),
				Err(_) => Err(InvalidTransaction::Payment.into()),
			}
		}
	}

	/// Hand the fee and the tip over to the `[OnUnbalanced]` implementation.
	/// Since the predicted fee might have been too high, parts of the fee may
	/// be refunded.
	///
	/// Note: The `corrected_fee` already includes the `tip`.
	fn correct_and_deposit_fee(
		_who: &T::AccountId,
		_dispatch_info: &DispatchInfoOf<T::RuntimeCall>,
		_post_info: &PostDispatchInfoOf<T::RuntimeCall>,
		_corrected_fee: Self::Balance,
		_tip: Self::Balance,
		_already_withdrawn: Self::LiquidityInfo,
	) -> Result<(), TransactionValidityError> {
		// if let Some(paid) = already_withdrawn {
		// 	// Calculate how much refund we should return
		// 	let refund_amount = paid.peek().saturating_sub(corrected_fee);
		// 	// refund to the the account that paid the fees. If this fails, the
		// 	// account might have dropped below the existential balance. In
		// 	// that case we don't refund anything.

		// 	let refund_imbalance = C::deposit_into_existing(&who, refund_amount)
		// 		.unwrap_or_else(|_| C::PositiveImbalance::zero());
		// 	// merge the imbalance caused by paying the fees and refunding parts of it again.
		// 	let adjusted_paid = paid
		// 		.offset(refund_imbalance)
		// 		.same()
		// 		.map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Payment))?;
		// 	// Call someone else to handle the imbalance (fee and tip separately)
		// 	let (tip, fee) = adjusted_paid.split(tip);
		// 	OU::on_unbalanceds(Some(fee).into_iter().chain(Some(tip)));
		// }
		Ok(())
	}
}
