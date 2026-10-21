CREATE TABLE system_branding (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    logo_url TEXT NOT NULL,
    system_name TEXT NOT NULL,
    login_title TEXT NOT NULL,
    system_description TEXT NOT NULL,
    filing_html TEXT NOT NULL
);
