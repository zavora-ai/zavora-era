-- Track how far each fixed asset has been depreciated so a depreciation run is
-- idempotent (cannot double-post a period) and can catch up missed months.
-- NULL = never depreciated.
ALTER TABLE fixed_assets ADD COLUMN IF NOT EXISTS depreciated_through DATE;

-- Backfill existing assets that already have accumulated depreciation: assume
-- they were depreciated through the end of the month before "now" is unknown, so
-- leave NULL and let the first run catch up from acquisition. (Fresh installs are
-- unaffected.)
