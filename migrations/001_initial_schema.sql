-- Initial database schema

-- Libraries table
CREATE TABLE IF NOT EXISTS libraries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_libraries_lower_name ON libraries(LOWER(name));

-- Versions table
CREATE TABLE IF NOT EXISTS versions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    name TEXT NOT NULL DEFAULT '',
    
    -- Status tracking
    status TEXT NOT NULL DEFAULT 'not_indexed',
    progress_pages INTEGER NOT NULL DEFAULT 0,
    progress_max_pages INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    started_at DATETIME,
    updated_at DATETIME,
    
    -- Scraper options
    source_url TEXT,
    scraper_options JSON,
    
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    
    UNIQUE(library_id, name)
);

CREATE INDEX IF NOT EXISTS idx_versions_status ON versions(status);
CREATE INDEX IF NOT EXISTS idx_versions_library_status ON versions(library_id, status);
CREATE INDEX IF NOT EXISTS idx_versions_source_url ON versions(source_url);
CREATE INDEX IF NOT EXISTS idx_versions_library_lower_name ON versions(library_id, LOWER(name));

-- Pages table
CREATE TABLE IF NOT EXISTS pages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    version_id INTEGER NOT NULL REFERENCES versions(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    title TEXT,
    etag TEXT,
    last_modified TEXT,
    content_type TEXT,
    depth INTEGER NOT NULL DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    
    UNIQUE(version_id, url)
);

CREATE INDEX IF NOT EXISTS idx_pages_version_id ON pages(version_id);
CREATE INDEX IF NOT EXISTS idx_pages_url ON pages(url);
CREATE INDEX IF NOT EXISTS idx_pages_etag ON pages(etag) WHERE etag IS NOT NULL;

-- Documents table
CREATE TABLE IF NOT EXISTS documents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    page_id INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    metadata JSON,
    sort_order INTEGER NOT NULL DEFAULT 0,
    embedding BLOB,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_documents_page_id ON documents(page_id);
CREATE INDEX IF NOT EXISTS idx_documents_sort_order ON documents(page_id, sort_order);

-- Full-text search virtual table
CREATE VIRTUAL TABLE IF NOT EXISTS documents_fts USING fts5(
    content,
    title,
    url,
    path,
    tokenize='porter unicode61',
    content=documents,
    content_rowid=id
);

-- Triggers to keep FTS in sync
CREATE TRIGGER IF NOT EXISTS documents_fts_after_insert
AFTER INSERT ON documents
BEGIN
    INSERT INTO documents_fts(rowid, content, title, url, path)
    SELECT 
        NEW.id,
        NEW.content,
        p.title,
        p.url,
        COALESCE(json_extract(NEW.metadata, '$.path'), '[]')
    FROM pages p
    WHERE p.id = NEW.page_id;
END;

CREATE TRIGGER IF NOT EXISTS documents_fts_after_update
AFTER UPDATE ON documents
BEGIN
    DELETE FROM documents_fts WHERE rowid = OLD.id;
    INSERT INTO documents_fts(rowid, content, title, url, path)
    SELECT 
        NEW.id,
        NEW.content,
        p.title,
        p.url,
        COALESCE(json_extract(NEW.metadata, '$.path'), '[]')
    FROM pages p
    WHERE p.id = NEW.page_id;
END;

CREATE TRIGGER IF NOT EXISTS documents_fts_after_delete
AFTER DELETE ON documents
BEGIN
    DELETE FROM documents_fts WHERE rowid = OLD.id;
END;
