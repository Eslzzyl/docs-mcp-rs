-- Create vector search virtual table for sqlite-vec extension

CREATE VIRTUAL TABLE IF NOT EXISTS documents_vec USING vec0(
    embedding FLOAT[1536]
);
