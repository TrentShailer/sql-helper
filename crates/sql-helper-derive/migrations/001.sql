CREATE TABLE IF NOT EXISTS challenges (
  challenge BYTEA NOT NULL PRIMARY KEY,
  origin VARCHAR NOT NULL,
  issued TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT (timezone('utc', NOW())),
  expires TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT (timezone('utc', NOW()) + '15 minutes'::INTERVAL)
);
