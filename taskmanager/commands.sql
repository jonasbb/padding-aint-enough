/* Reset aborted tasks back to initial state, but only for groupid=0 (main group) */
UPDATE
    tasks
SET
    aborted = FALSE,
    restart_count = 0,
    state = 'created'
WHERE
    DOMAIN IN (
        SELECT
            *
        FROM (
            SELECT
                DISTINCT DOMAIN
            FROM
                tasks
            WHERE
                aborted = TRUE) AS s
        ORDER BY
            random()
        LIMIT 400)
    AND groupid = 0;

