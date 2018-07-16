table! {
    use diesel::sql_types::*;
    use models::TaskStateMapping;

    /// Representation of the `tasks` table.
    ///
    /// (Automatically generated by Diesel.)
    tasks (id) {
        /// The `id` column of the `tasks` table.
        ///
        /// Its SQL type is `Integer`.
        ///
        /// (Automatically generated by Diesel.)
        id -> Integer,
        /// The `priority` column of the `tasks` table.
        ///
        /// Its SQL type is `Integer`.
        ///
        /// (Automatically generated by Diesel.)
        priority -> Integer,
        /// The `name` column of the `tasks` table.
        ///
        /// Its SQL type is `Text`.
        ///
        /// (Automatically generated by Diesel.)
        name -> Text,
        /// The `domain` column of the `tasks` table.
        ///
        /// Its SQL type is `Text`.
        ///
        /// (Automatically generated by Diesel.)
        domain -> Text,
        /// The `domain_counter` column of the `tasks` table.
        ///
        /// Its SQL type is `Integer`.
        ///
        /// (Automatically generated by Diesel.)
        domain_counter -> Integer,
        /// The `state` column of the `tasks` table.
        ///
        /// Its SQL type is `Text`.
        ///
        /// (Automatically generated by Diesel.)
        state -> TaskStateMapping,
        /// The `restart_count` column of the `tasks` table.
        ///
        /// Its SQL type is `Integer`.
        ///
        /// (Automatically generated by Diesel.)
        restart_count -> Integer,
        /// The `last_modified` column of the `tasks` table.
        ///
        /// Its SQL type is `Timestamp`.
        ///
        /// (Automatically generated by Diesel.)
        last_modified -> Timestamp,
        /// The `associated_data` column of the `tasks` table.
        ///
        /// Its SQL type is `Nullable<Text>`.
        ///
        /// (Automatically generated by Diesel.)
        associated_data -> Nullable<Text>,
    }
}
