-- Fix FTS5 delete trigger to use correct syntax
-- FTS5 virtual tables require INSERT ... VALUES('delete', ...) syntax

-- Drop existing incorrect trigger
DROP TRIGGER IF EXISTS documents_fts_after_delete;

-- Create corrected trigger
CREATE TRIGGER documents_fts_after_delete
AFTER DELETE ON documents
BEGIN
    INSERT INTO documents_fts(documents_fts, rowid, content, title, url, path)
    VALUES('delete', OLD.id, '', '', '', '');
END;
