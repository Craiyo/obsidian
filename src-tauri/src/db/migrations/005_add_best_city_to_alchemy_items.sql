-- Add best_city column to alchemy_session_items so we can recommend crafting city per item
ALTER TABLE alchemy_session_items ADD COLUMN best_city TEXT;

-- No indexes required for this small column
