UPDATE users
SET roles = (
    SELECT jsonb_agg(DISTINCT value)
    FROM jsonb_array_elements(roles || '["Admin", "Proctor"]'::jsonb) as value
)
WHERE email = 'corrado@mazzarelli.biz';
