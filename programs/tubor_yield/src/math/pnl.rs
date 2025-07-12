/// Example of using the PnL trait
///
/// ```rust
/// use tubor_yield::math::pnl::PnL;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let entry_price = 100_u64;
///     let current_price = 110_u64;
///     let position_size = 1000_u64;
///     let pnl = position_size.calculate_pnl(entry_price, current_price).map_err(|e| format!("{:?}", e))?;
///     Ok(())
/// }
/// ```
pub fn example_pnl_usage() -> Result<(), Box<dyn std::error::Error>> {
    let entry_price = 100_u64;
    let current_price = 110_u64;
    let position_size = 1000_u64;
    let pnl = position_size
        .calculate_pnl(entry_price, current_price)
        .map_err(|e| format!("{:?}", e))?;
    Ok(())
}
