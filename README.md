# **Sniper Algorithm**

The **Sniper Algorithm** is a cryptocurrency trading algorithm written in Rust. It is designed to execute trades on the Binance Testnet.

---

## **Setup and Usage**

### **1. Prerequisites**
- Install Rust and Cargo: [Rust Installation Guide](https://www.rust-lang.org/tools/install)

### **2. Configure Your Environment**
Set the following environment variables in your `.env` file or directly in your shell:

API_KEY="8Rz2XDFFItHESqPZ7CUl3Gk4JyQU73jBsqMAEhOKaizD4Fx28ryWNQuOg9OMweND"
API_SECRET="3Bo4CXMF95vj17hzREUxIzLP5wQfueU8YMplVREKr1pGQqCkBz088XOCw4N8mGV6"

> **Note**: You can generate your own Binance Testnet keys or use the ones provided above.

---

### **3. Configure `config.toml`**

Edit the `config.toml` file in the project's configuration directory to define your algorithm parameters. Below is an example configuration with multiple algorithms:

```toml
[[algorithms]]
algo_type = "sniper"
algo_id = "5552"
side = "sell"
quantity = 0.005
price = 92100
base = "btc"
quote = "usdt"

[[algorithms]]
algo_type = "sniper"
algo_id = "343555"
side = "buy"
quantity = 0.5
price = 200
base = "sol"
quote = "usdt"

[[algorithms]]
algo_type = "sniper"
algo_id = "555333"
side = "buy"
quantity = 0.5
price = 3000
base = "eth"
quote = "usdt"

```

#### **Explanation of Parameters**
- **`algo_type`**: Currently supports only `"sniper"`.
- **`algo_id`**: Unique identifier for the algorithm, used to track it in the system.
- **`side`**: `"buy"` or `"sell"`, indicating the order type.
- **`quantity`**: The quantity to buy or sell.
- **`price`**: The maximum price at which the order can be executed.
- **`base`**: The base asset of the trading pair (e.g., `btc` for `BTC/USDT`).
- **`quote`**: The quote asset of the trading pair (e.g., `usdt` for `BTC/USDT`).


#### **Finding Trading Pairs and Prices**
You can find available trading pairs and their current prices on Binance market page:
[Binance Spot Markets - USDT](https://www.binance.com/en/markets/spot_margin-USDT).

Use this resource to determine valid trading pairs and ensure your configuration aligns with market prices.

---

### **4. Run the Algorithm**
To start the algorithm, run the following command in the project directory:

cargo run

---

## **Key Features**
- Supports multiple algorithms running simultaneously on the same or different trading pairs.
- Simple configuration via `config.toml`.
- Optimized for high-performance trading on the Binance Testnet.

---

## **Disclaimer**
This project is for educational purposes only and is designed to work with the Binance Testnet. Use it with caution and do not use real funds without thorough testing.
