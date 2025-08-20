







use anchor_lang::prelude::*;
use anchor_lang::solana_program::pubkey::Pubkey;

declare_id!("FqzkXZdwYjurnUKetJCAvaUw5WAqbwzU6gZEwydeEfqS");

const STAKING_APY_BASIS_POINTS: u64 = 500; // 5% APY
const LENDING_INTEREST_RATE: u64 = 13; // 13% interest rate
const PERCENTAGE_DIVISOR: u64 = 100;
const BASIS_POINTS_DIVISOR: u64 = 10000;
const COLLATERAL_RATIO: u64 = 80; // 80% collateral requirement
const SLOTS_PER_YEAR: u64 = 432000 * 365;
const MAX_DEPOSIT_AMOUNT: u64 = 1_000_000 * 1_000_000_000; // 1M tokens with 9 decimals
const INITIAL_BANK_BALANCE: u64 = 5000 * 1_000_000_000; // 5000 tokens with 9 decimals

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid address provided")]
    InvalidAddress,
    #[msg("Amount must be greater than zero")]
    InvalidAmount,
    #[msg("Insufficient balance for this operation")]
    InsufficientBalance,
    #[msg("User not eligible for this operation")]
    NotEligible,
    #[msg("Bank has insufficient funds")]
    BankInsufficientFunds,
    #[msg("Staking period too short")]
    StakingPeriodTooShort,
    #[msg("Arithmetic overflow detected")]
    ArithmeticOverflow,
    #[msg("Amount exceeds maximum allowed")]
    AmountTooLarge,
    #[msg("Unauthorized access")]
    Unauthorized,
    #[msg("Invalid collateral ratio")]
    InvalidCollateralRatio,
    #[msg("Bank already initialized")]
    BankAlreadyInitialized,
    #[msg("User already has active loan")]
    ActiveLoanExists,
    #[msg("No active loan found")]
    NoActiveLoan,
    #[msg("Minimum staking period not met")]
    MinimumStakingPeriodNotMet,
}

#[program]
pub mod banking {
    use super::*;

    /// Initialize the bank with admin controls
    pub fn initialize_bank(ctx: Context<InitializeBank>) -> Result<()> {
        let bank = &mut ctx.accounts.bank_account;
        bank.admin = ctx.accounts.admin.key();
        bank.balance = INITIAL_BANK_BALANCE;
        bank.lent_balance = 0;
        bank.staked_balance = 0;
        bank.total_users = 0;
        bank.is_operational = true;
        
        msg!("Bank initialized with admin: {}", ctx.accounts.admin.key());
        Ok(())
    }

    /// Create a new user account
    pub fn create_user(ctx: Context<InitializeUser>) -> Result<()> {
        let user = &mut ctx.accounts.user_account;
        let bank = &mut ctx.accounts.bank_account;
        
        user.balance = 0;
        user.staked_balance = 0;
        user.lent_balance = 0;
        user.stake_slot = 0;
        user.loan_timestamp = 0;
        user.owner = ctx.accounts.payer.key();
        
        // Update bank statistics
        bank.total_users = bank.total_users
            .checked_add(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        msg!("User account created for: {}", ctx.accounts.payer.key());
        Ok(())
    }

    /// Delete user account (close account and return rent)
    pub fn delete_user(ctx: Context<DeleteUser>) -> Result<()> {
        let user = &ctx.accounts.user_account;
        let bank = &mut ctx.accounts.bank_account;
        
        // Ensure user has no active balances
        if user.balance > 0 || user.staked_balance > 0 || user.lent_balance > 0 {
            return Err(ErrorCode::InsufficientBalance.into());
        }
        
        // Update bank statistics
        bank.total_users = bank.total_users
            .checked_sub(1)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        msg!("User account deleted for: {}", ctx.accounts.payer.key());
        Ok(())
    }

    /// Deposit funds into user account
    pub fn deposit(ctx: Context<Operations>, amount: u64) -> Result<()> {
        // Validate input
        if amount <= 0 {
            return Err(ErrorCode::InvalidAmount.into());
        }
        
        if amount > MAX_DEPOSIT_AMOUNT {
            return Err(ErrorCode::AmountTooLarge.into());
        }
        
        let user = &mut ctx.accounts.user_account;
        let bank = &ctx.accounts.bank_account;
        
        // Check if bank is operational
        if !bank.is_operational {
            return Err(ErrorCode::NotEligible.into());
        }
        
        // Update user balance with overflow protection
        user.balance = user.balance
            .checked_add(amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        emit!(DepositEvent {
            user: ctx.accounts.payer.key(),
            amount,
            new_balance: user.balance,
        });
        
        msg!("Deposited {} tokens for user: {}", amount, ctx.accounts.payer.key());
        Ok(())
    }

    /// Withdraw funds from user account
    pub fn withdraw(ctx: Context<Operations>, amount: u64) -> Result<()> {
        // Validate input
        if amount == 0 {
            return Err(ErrorCode::InvalidAmount.into());
        }
        
        let user = &mut ctx.accounts.user_account;
        let bank = &ctx.accounts.bank_account;
        
        // Check if bank is operational
        if !bank.is_operational {
            return Err(ErrorCode::NotEligible.into());
        }
        
        // Check sufficient balance
        if user.balance < amount {
            return Err(ErrorCode::InsufficientBalance.into());
        }
        
        // Update user balance with underflow protection
        user.balance = user.balance
            .checked_sub(amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        emit!(WithdrawEvent {
            user: ctx.accounts.payer.key(),
            amount,
            new_balance: user.balance,
        });
        
        msg!("Withdrew {} tokens for user: {}", amount, ctx.accounts.payer.key());
        Ok(())
    }

    /// Check user balance and emit event
    pub fn check_balance(ctx: Context<CheckBalance>) -> Result<()> {
        let user = &ctx.accounts.user_account;
        
        emit!(BalanceChecked {
            user: user.owner,
            balance: user.balance,
            staked_balance: user.staked_balance,
            lent_balance: user.lent_balance,
        });
        
        Ok(())
    }

    /// Stake tokens for rewards
    pub fn stake(ctx: Context<Staking>, amount: u64) -> Result<()> {
        // Validate input
        if amount <= 0 {
            return Err(ErrorCode::InvalidAmount.into());
        }
        
        let user = &mut ctx.accounts.user_account;
        let bank = &mut ctx.accounts.bank_account;
        
        // Check if bank is operational
        if !bank.is_operational {
            return Err(ErrorCode::NotEligible.into());
        }
        
        // Check sufficient balance
        if user.balance < amount {
            return Err(ErrorCode::InsufficientBalance.into());
        }
        
        // If user already has staked balance, calculate and add rewards first
        if user.staked_balance > 0 {
            let reward = calculate_staking_reward(
                user.staked_balance,
                ctx.accounts.clock.slot,
                user.stake_slot
            )?;
            
            if reward > 0 {
                // Check if bank can pay reward
                if bank.balance < reward {
                    return Err(ErrorCode::BankInsufficientFunds.into());
                }
                
                user.balance = user.balance
                    .checked_add(reward)
                    .ok_or(ErrorCode::ArithmeticOverflow)?;
                bank.balance = bank.balance
                    .checked_sub(reward)
                    .ok_or(ErrorCode::ArithmeticOverflow)?;
            }
        }
        
        // Update staking information
        user.stake_slot = ctx.accounts.clock.slot;
        user.balance = user.balance
            .checked_sub(amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        user.staked_balance = user.staked_balance
            .checked_add(amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        bank.staked_balance = bank.staked_balance
            .checked_add(amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        emit!(StakeEvent {
            user: ctx.accounts.payer.key(),
            amount,
            total_staked: user.staked_balance,
        });
        
        msg!("Staked {} tokens for user: {}", amount, ctx.accounts.payer.key());
        Ok(())
    }

    /// Unstake tokens and claim rewards
    pub fn unstake(ctx: Context<Staking>, amount: u64) -> Result<()> {
        // Validate input
        if amount == 0 {
            return Err(ErrorCode::InvalidAmount.into());
        }
        
        let user = &mut ctx.accounts.user_account;
        let bank = &mut ctx.accounts.bank_account;
        
        // Check if user has sufficient staked balance
        if user.staked_balance < amount {
            return Err(ErrorCode::InsufficientBalance.into());
        }
        
        // Calculate rewards
        let reward = calculate_staking_reward(
            amount,
            ctx.accounts.clock.slot,
            user.stake_slot
        )?;
        
        // Check if bank can pay reward
        if bank.balance < reward {
            return Err(ErrorCode::BankInsufficientFunds.into());
        }
        
        // Update balances
        user.staked_balance = user.staked_balance
            .checked_sub(amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        user.balance = user.balance
            .checked_add(amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_add(reward)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        bank.staked_balance = bank.staked_balance
            .checked_sub(amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        bank.balance = bank.balance
            .checked_sub(reward)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        emit!(UnstakeEvent {
            user: ctx.accounts.payer.key(),
            amount,
            reward,
            remaining_staked: user.staked_balance,
        });
        
        msg!("Unstaked {} tokens with {} reward for user: {}", amount, reward, ctx.accounts.payer.key());
        Ok(())
    }

    /// Borrow funds from the bank
    pub fn borrow(ctx: Context<LoanOperations>, amount: u64) -> Result<()> {
        // Validate input
        if amount == 0 {
            return Err(ErrorCode::InvalidAmount.into());
        }
        
        let user = &mut ctx.accounts.user_account;
        let bank = &mut ctx.accounts.bank_account;
        
        // Check if bank is operational
        if !bank.is_operational {
            return Err(ErrorCode::NotEligible.into());
        }
        
        // Check if user already has an active loan
        if user.lent_balance > 0 {
            return Err(ErrorCode::ActiveLoanExists.into());
        }
        
        // Check if bank has sufficient funds
        if bank.balance < amount {
            return Err(ErrorCode::BankInsufficientFunds.into());
        }
        
        // Calculate maximum borrowing amount based on collateral (80% of balance)
        let max_borrow = user.balance
            .checked_mul(COLLATERAL_RATIO)
            .ok_or(ErrorCode::ArithmeticOverflow)?
            .checked_div(PERCENTAGE_DIVISOR)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        if amount > max_borrow {
            return Err(ErrorCode::InvalidCollateralRatio.into());
        }
        
        // Update balances
        user.lent_balance = amount;
        user.balance = user.balance
            .checked_add(amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        user.loan_timestamp = ctx.accounts.clock.unix_timestamp;
        bank.balance = bank.balance
            .checked_sub(amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        bank.lent_balance = bank.lent_balance
            .checked_add(amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        emit!(BorrowEvent {
            user: ctx.accounts.payer.key(),
            amount,
            collateral_used: user.balance.checked_sub(amount).unwrap_or(0),
        });
        
        msg!("Borrowed {} tokens for user: {}", amount, ctx.accounts.payer.key());
        Ok(())
    }

    /// Repay loan with interest
    pub fn repay_loan(ctx: Context<LoanOperations>) -> Result<()> {
        let user = &mut ctx.accounts.user_account;
        let bank = &mut ctx.accounts.bank_account;
        
        // Check if user has an active loan
        if user.lent_balance == 0 {
            return Err(ErrorCode::NoActiveLoan.into());
        }
        
        // Calculate interest based on time elapsed
        let time_elapsed = ctx.accounts.clock.unix_timestamp
            .checked_sub(user.loan_timestamp)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        // Calculate interest (simple interest for demonstration)
        let interest = calculate_loan_interest(user.lent_balance, time_elapsed)?;
        let total_repayment = user.lent_balance
            .checked_add(interest)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        // Check if user has sufficient balance to repay
        if user.balance < total_repayment {
            return Err(ErrorCode::InsufficientBalance.into());
        }
        
        // Update balances
        let principal = user.lent_balance;
        bank.lent_balance = bank.lent_balance
            .checked_sub(principal)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        bank.balance = bank.balance
            .checked_add(total_repayment)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        user.balance = user.balance
            .checked_sub(total_repayment)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        user.lent_balance = 0;
        user.loan_timestamp = 0;
        
        emit!(RepayEvent {
            user: ctx.accounts.payer.key(),
            principal,
            interest,
            total_repayment,
        });
        
        msg!("Repaid loan: {} principal + {} interest for user: {}", principal, interest, ctx.accounts.payer.key());
        Ok(())
    }

    /// Transfer funds between users
    pub fn transfer_funds(ctx: Context<FundTransfer>, amount: u64) -> Result<()> {
        // Validate input
        if amount == 0 {
            return Err(ErrorCode::InvalidAmount.into());
        }
        
        let from_user = &mut ctx.accounts.from_user;
        let to_user = &mut ctx.accounts.to_user;
        
        // Check sufficient balance
        if from_user.balance < amount {
            return Err(ErrorCode::InsufficientBalance.into());
        }
        
        // Prevent self-transfer
        if from_user.key() == to_user.key() {
            return Err(ErrorCode::InvalidAddress.into());
        }
        
        // Update balances
        from_user.balance = from_user.balance
            .checked_sub(amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        to_user.balance = to_user.balance
            .checked_add(amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        emit!(TransferEvent {
            from: from_user.owner,
            to: to_user.owner,
            amount,
        });
        
        msg!("Transferred {} tokens from {} to {}", amount, from_user.owner, to_user.owner);
        Ok(())
    }

    /// Admin function to toggle bank operational status
    pub fn toggle_bank_status(ctx: Context<AdminOperation>) -> Result<()> {
        let bank = &mut ctx.accounts.bank_account;
        
        // Check if caller is admin
        if ctx.accounts.admin.key() != bank.admin {
            return Err(ErrorCode::Unauthorized.into());
        }
        
        bank.is_operational = !bank.is_operational;
        
        emit!(BankStatusChanged {
            admin: ctx.accounts.admin.key(),
            is_operational: bank.is_operational,
        });
        
        msg!("Bank operational status changed to: {}", bank.is_operational);
        Ok(())
    }

    /// Admin function to add funds to bank
    pub fn add_bank_funds(ctx: Context<AdminOperation>, amount: u64) -> Result<()> {
        let bank = &mut ctx.accounts.bank_account;
        
        // Check if caller is admin
        if ctx.accounts.admin.key() != bank.admin {
            return Err(ErrorCode::Unauthorized.into());
        }
        
        if amount == 0 {
            return Err(ErrorCode::InvalidAmount.into());
        }
        
        bank.balance = bank.balance
            .checked_add(amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;
        
        emit!(BankFundsAdded {
            admin: ctx.accounts.admin.key(),
            amount,
            new_balance: bank.balance,
        });
        
        msg!("Added {} tokens to bank balance", amount);
        Ok(())
    }
}

// Helper functions
fn calculate_staking_reward(staked_amount: u64, current_slot: u64, stake_slot: u64) -> Result<u64> {
    let slots_staked = current_slot
        .checked_sub(stake_slot)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    // Calculate reward based on APY
    let reward = staked_amount
        .checked_mul(STAKING_APY_BASIS_POINTS)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_mul(slots_staked)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(BASIS_POINTS_DIVISOR)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(SLOTS_PER_YEAR)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    Ok(reward)
}

fn calculate_loan_interest(principal: u64, time_elapsed_seconds: i64) -> Result<u64> {
    if time_elapsed_seconds <= 0 {
        return Ok(0);
    }
    
    let time_elapsed = time_elapsed_seconds as u64;
    let seconds_per_year = 365 * 24 * 60 * 60;
    
    // Calculate simple interest
    let interest = principal
        .checked_mul(LENDING_INTEREST_RATE)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_mul(time_elapsed)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(PERCENTAGE_DIVISOR)
        .ok_or(ErrorCode::ArithmeticOverflow)?
        .checked_div(seconds_per_year)
        .ok_or(ErrorCode::ArithmeticOverflow)?;
    
    Ok(interest)
}

// Account structures
#[derive(Accounts)]
pub struct InitializeBank<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        space = 8 + Bank::INIT_SPACE,
        payer = admin,
        seeds = [b"bank"],
        bump
    )]
    pub bank_account: Account<'info, Bank>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeUser<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"bank"],
        bump
    )]
    pub bank_account: Account<'info, Bank>,

    #[account(
        init,
        payer = payer,
        space = 8 + User::INIT_SPACE,
        seeds = [b"user", payer.key().as_ref()],
        bump,
    )]
    pub user_account: Account<'info, User>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DeleteUser<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"bank"],
        bump
    )]
    pub bank_account: Account<'info, Bank>,

    #[account(
        mut,
        seeds = [b"user", payer.key().as_ref()],
        bump,
        close = payer,
    )]
    pub user_account: Account<'info, User>,
}

#[derive(Accounts)]
pub struct CheckBalance<'info> {
    #[account(
        seeds = [b"user", user_account.owner.as_ref()],
        bump
    )]
    pub user_account: Account<'info, User>,
}

#[derive(Accounts)]
pub struct Operations<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"bank"],
        bump
    )]
    pub bank_account: Account<'info, Bank>,

    #[account(
        mut,
        seeds = [b"user", payer.key().as_ref()],
        bump,
        constraint = user_account.owner == payer.key() @ ErrorCode::Unauthorized
    )]
    pub user_account: Account<'info, User>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct LoanOperations<'info> {
    pub clock: Sysvar<'info, Clock>,

    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"bank"],
        bump
    )]
    pub bank_account: Account<'info, Bank>,

    #[account(
        mut,
        seeds = [b"user", payer.key().as_ref()],
        bump,
        constraint = user_account.owner == payer.key() @ ErrorCode::Unauthorized
    )]
    pub user_account: Account<'info, User>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Staking<'info> {
    pub clock: Sysvar<'info, Clock>,

    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"bank"],
        bump
    )]
    pub bank_account: Account<'info, Bank>,

    #[account(
        mut,
        seeds = [b"user", payer.key().as_ref()],
        bump,
        constraint = user_account.owner == payer.key() @ ErrorCode::Unauthorized
    )]
    pub user_account: Account<'info, User>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct FundTransfer<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"user", payer.key().as_ref()],
        bump,
        constraint = from_user.owner == payer.key() @ ErrorCode::Unauthorized
    )]
    pub from_user: Account<'info, User>,

    #[account(
        mut,
        seeds = [b"user", to_user.owner.as_ref()],
        bump,
    )]
    pub to_user: Account<'info, User>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AdminOperation<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [b"bank"],
        bump,
        constraint = bank_account.admin == admin.key() @ ErrorCode::Unauthorized
    )]
    pub bank_account: Account<'info, Bank>,

    pub system_program: Program<'info, System>,
}

// Data structures
#[account]
#[derive(InitSpace)]
pub struct Bank {
    pub admin: Pubkey,
    pub balance: u64,
    pub staked_balance: u64,
    pub lent_balance: u64,
    pub total_users: u64,
    pub is_operational: bool,
}

#[account]
#[derive(InitSpace)]
pub struct User {
    pub owner: Pubkey,
    pub balance: u64,
    pub staked_balance: u64,
    pub stake_slot: u64,
    pub lent_balance: u64,
    pub loan_timestamp: i64,
}

// Events
#[event]
pub struct BalanceChecked {
    pub user: Pubkey,
    pub balance: u64,
    pub staked_balance: u64,
    pub lent_balance: u64,
}

#[event]
pub struct DepositEvent {
    pub user: Pubkey,
    pub amount: u64,
    pub new_balance: u64,
}

#[event]
pub struct WithdrawEvent {
    pub user: Pubkey,
    pub amount: u64,
    pub new_balance: u64,
}

#[event]
pub struct StakeEvent {
    pub user: Pubkey,
    pub amount: u64,
    pub total_staked: u64,
}

#[event]
pub struct UnstakeEvent {
    pub user: Pubkey,
    pub amount: u64,
    pub reward: u64,
    pub remaining_staked: u64,
}

#[event]
pub struct BorrowEvent {
    pub user: Pubkey,
    pub amount: u64,
    pub collateral_used: u64,
}

#[event]
pub struct RepayEvent {
    pub user: Pubkey,
    pub principal: u64,
    pub interest: u64,
    pub total_repayment: u64,
}

#[event]
pub struct TransferEvent {
    pub from: Pubkey,
    pub to: Pubkey,
    pub amount: u64,
}

#[event]
pub struct BankStatusChanged {
    pub admin: Pubkey,
    pub is_operational: bool,
}

#[event]
pub struct BankFundsAdded {
    pub admin: Pubkey,
    pub amount: u64,
    pub new_balance: u64,
}