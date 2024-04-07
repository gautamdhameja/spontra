use pallet_transaction_payment::Config;
use pallet_transaction_payment::OnChargeTransaction;
use sp_runtime::AccountId32;
use core::marker::PhantomData;
use codec::Decode;

use hex_literal::hex;

use sp_runtime::{
	traits::{DispatchInfoOf, PostDispatchInfoOf, Saturating, Zero},
	transaction_validity::InvalidTransaction,
};

use frame_support::{
	traits::{Currency, ExistenceRequirement, WithdrawReasons,
		Imbalance, OnUnbalanced,},
	unsigned::TransactionValidityError,
};

// pub struct SponsoredFungibleAdapter<F, OU>(PhantomData<(F, OU)>);

// impl<T, F, OU> OnChargeTransaction<T> for SponsoredFungibleAdapter<F, OU>
// where
// 	T: Config,
// 	F: Balanced<T::AccountId>,
// 	OU: OnUnbalanced<Credit<T::AccountId, F>>,
// {
// 	type LiquidityInfo = Option<Credit<T::AccountId, F>>;
// 	type Balance = <F as Inspect<<T as frame_system::Config>::AccountId>>::Balance;

// 	fn withdraw_fee(
// 		_who: &<T>::AccountId,
// 		_call: &<T>::RuntimeCall,
// 		_dispatch_info: &DispatchInfoOf<<T>::RuntimeCall>,
// 		fee: Self::Balance,
// 		_tip: Self::Balance,
// 	) -> Result<Self::LiquidityInfo, TransactionValidityError> {
// 		if fee.is_zero() {
// 			return Ok(None)
// 		}

// 		let account32: AccountId32 = hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"].into();
// 		let mut init_account32 = AccountId32::as_ref(&account32);
// 		let payer: <T>::AccountId = <T as frame_system::Config>::AccountId::decode(&mut init_account32).unwrap();

// 		match F::withdraw(
// 			&payer,
// 			fee,
// 			Precision::Exact,
// 			frame_support::traits::tokens::Preservation::Preserve,
// 			frame_support::traits::tokens::Fortitude::Polite,
// 		) {
// 			Ok(imbalance) => Ok(Some(imbalance)),
// 			Err(_) => Err(InvalidTransaction::Payment.into()),
// 		}
// 	}

// 	fn correct_and_deposit_fee(
// 		who: &<T>::AccountId,
// 		_dispatch_info: &DispatchInfoOf<<T>::RuntimeCall>,
// 		_post_info: &PostDispatchInfoOf<<T>::RuntimeCall>,
// 		corrected_fee: Self::Balance,
// 		tip: Self::Balance,
// 		already_withdrawn: Self::LiquidityInfo,
// 	) -> Result<(), TransactionValidityError> {
// 		if let Some(paid) = already_withdrawn {
// 			// Calculate how much refund we should return
// 			let refund_amount = paid.peek().saturating_sub(corrected_fee);
// 			// refund to the the account that paid the fees if it exists. otherwise, don't refind
// 			// anything.
// 			let refund_imbalance = if F::total_balance(who) > F::Balance::zero() {
// 				F::deposit(who, refund_amount, Precision::BestEffort)
// 					.unwrap_or_else(|_| Debt::<T::AccountId, F>::zero())
// 			} else {
// 				Debt::<T::AccountId, F>::zero()
// 			};
// 			// merge the imbalance caused by paying the fees and refunding parts of it again.
// 			let adjusted_paid: Credit<T::AccountId, F> = paid
// 				.offset(refund_imbalance)
// 				.same()
// 				.map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Payment))?;
// 			// Call someone else to handle the imbalance (fee and tip separately)
// 			let (tip, fee) = adjusted_paid.split(tip);
// 			OU::on_unbalanceds(Some(fee).into_iter().chain(Some(tip)));
// 		}

// 		Ok(())
// 	}
// }

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
{
	type LiquidityInfo = Option<NegativeImbalanceOf<C, T>>;
	type Balance = <C as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	/// Withdraw the predicted fee from the transaction origin.
	///
	/// Note: The `fee` already includes the `tip`.
	fn withdraw_fee(
		_who: &T::AccountId,
		_call: &T::RuntimeCall,
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

		let account32: AccountId32 = hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"].into();
		let mut init_account32 = AccountId32::as_ref(&account32);
		let payer: <T>::AccountId = <T as frame_system::Config>::AccountId::decode(&mut init_account32).unwrap();

		match C::withdraw(&payer, fee, withdraw_reason, ExistenceRequirement::KeepAlive) {
			Ok(imbalance) => Ok(Some(imbalance)),
			Err(_) => Err(InvalidTransaction::Payment.into()),
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
		corrected_fee: Self::Balance,
		tip: Self::Balance,
		already_withdrawn: Self::LiquidityInfo,
	) -> Result<(), TransactionValidityError> {
		if let Some(paid) = already_withdrawn {
			// Calculate how much refund we should return
			let refund_amount = paid.peek().saturating_sub(corrected_fee);
			// refund to the the account that paid the fees. If this fails, the
			// account might have dropped below the existential balance. In
			// that case we don't refund anything.

			let account32: AccountId32 = hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"].into();
			let mut init_account32 = AccountId32::as_ref(&account32);
			let payer: <T>::AccountId = <T as frame_system::Config>::AccountId::decode(&mut init_account32).unwrap();

			let refund_imbalance = C::deposit_into_existing(&payer, refund_amount)
				.unwrap_or_else(|_| C::PositiveImbalance::zero());
			// merge the imbalance caused by paying the fees and refunding parts of it again.
			let adjusted_paid = paid
				.offset(refund_imbalance)
				.same()
				.map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Payment))?;
			// Call someone else to handle the imbalance (fee and tip separately)
			let (tip, fee) = adjusted_paid.split(tip);
			OU::on_unbalanceds(Some(fee).into_iter().chain(Some(tip)));
		}
		Ok(())
	}
}
