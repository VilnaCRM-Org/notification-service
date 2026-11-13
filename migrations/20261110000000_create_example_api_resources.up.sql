CREATE TABLE example_api_resources (
    id UUID NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    CONSTRAINT example_api_resources_pkey PRIMARY KEY (id),
    CONSTRAINT example_api_resources_name_check
        CHECK (char_length(name) BETWEEN 1 AND 255),
    CONSTRAINT example_api_resources_description_check
        CHECK (char_length(description) BETWEEN 1 AND 1024)
);
