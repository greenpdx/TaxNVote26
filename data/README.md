# Data Directory

Place `budauth.csv` (CBO Budget Authority CSV) in this directory.

Download from: https://www.cbo.gov/about/products/budget-economic-data

The server and budget-tree crate both reference this file for valid node IDs.

At runtime the server also writes `store.json` here (JSON file store for dev;
swap to PostgreSQL/SQLite via SQLx for production).
