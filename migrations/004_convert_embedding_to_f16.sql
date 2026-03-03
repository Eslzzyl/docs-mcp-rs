-- Convert embedding storage from f32 to f16 for 50% size reduction
-- This migration is handled specially in Rust code due to binary data conversion
-- See: src/store/migrations.rs

-- This file is kept for documentation purposes.
-- The actual conversion happens in the Rust migration code.
