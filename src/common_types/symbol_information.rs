use rust_decimal::Decimal;

pub struct SymbolInformation {
    pub min_quantity: Option<Decimal>,
    pub max_quantity: Option<Decimal>,
    pub lot_size: Option<Decimal>,
    pub min_price: Option<Decimal>,
    pub max_price: Option<Decimal>,
    pub tick_size: Option<Decimal>,
    pub min_amount: Option<Decimal>,
}

impl SymbolInformation {
    pub fn new() -> Self {
        SymbolInformation {
            min_quantity: None,
            max_quantity: None,
            lot_size: None,
            min_price: None,
            max_price: None,
            tick_size: None,
            min_amount: None,
        }
    }

    pub fn set_values(
        &mut self,
        min_quantity: Option<Decimal>,
        max_quantity: Option<Decimal>,
        lot_size: Option<Decimal>,
        min_price: Option<Decimal>,
        max_price: Option<Decimal>,
        tick_size: Option<Decimal>,
        min_amount: Option<Decimal>,
    ) {
        if let Some(value) = min_quantity {
            self.min_quantity = Some(value);
        }
        if let Some(value) = max_quantity {
            self.max_quantity = Some(value);
        }
        if let Some(value) = lot_size {
            self.lot_size = Some(value);
        }
        if let Some(value) = min_price {
            self.min_price = Some(value);
        }
        if let Some(value) = max_price {
            self.max_price = Some(value);
        }
        if let Some(value) = tick_size {
            self.tick_size = Some(value);
        }
        if let Some(value) = min_amount {
            self.min_amount = Some(value);
        }
    }
}
