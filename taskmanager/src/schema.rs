#![allow(proc_macro_derive_resolution_fallback, unused_imports)]

table! {
    use diesel::sql_types::*;
    use crate::models::Task_state;

    /// Representation of the `infos` table.
    ///
    /// (Automatically generated by Diesel.)
    infos (id) {
        /// The `id` column of the `infos` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        id -> Int4,
        /// The `task_id` column of the `infos` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        task_id -> Int4,
        /// The `time` column of the `infos` table.
        ///
        /// Its SQL type is `Timestamptz`.
        ///
        /// (Automatically generated by Diesel.)
        time -> Timestamptz,
        /// The `message` column of the `infos` table.
        ///
        /// Its SQL type is `Text`.
        ///
        /// (Automatically generated by Diesel.)
        message -> Text,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::models::Task_state;

    /// Representation of the `tasks` table.
    ///
    /// (Automatically generated by Diesel.)
    tasks (id) {
        /// The `id` column of the `tasks` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        id -> Int4,
        /// The `priority` column of the `tasks` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        priority -> Int4,
        /// The `name` column of the `tasks` table.
        ///
        /// Its SQL type is `Text`.
        ///
        /// (Automatically generated by Diesel.)
        name -> Text,
        /// The `website` column of the `tasks` table.
        ///
        /// Its SQL type is `Text`.
        ///
        /// (Automatically generated by Diesel.)
        website -> Text,
        /// The `website_counter` column of the `tasks` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        website_counter -> Int4,
        /// The `state` column of the `tasks` table.
        ///
        /// Its SQL type is `Task_state`.
        ///
        /// (Automatically generated by Diesel.)
        state -> Task_state,
        /// The `restart_count` column of the `tasks` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        restart_count -> Int4,
        /// The `aborted` column of the `tasks` table.
        ///
        /// Its SQL type is `Bool`.
        ///
        /// (Automatically generated by Diesel.)
        aborted -> Bool,
        /// The `last_modified` column of the `tasks` table.
        ///
        /// Its SQL type is `Timestamptz`.
        ///
        /// (Automatically generated by Diesel.)
        last_modified -> Timestamptz,
        /// The `associated_data` column of the `tasks` table.
        ///
        /// Its SQL type is `Nullable<Text>`.
        ///
        /// (Automatically generated by Diesel.)
        associated_data -> Nullable<Text>,
        /// The `groupid` column of the `tasks` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        groupid -> Int4,
        /// The `groupsize` column of the `tasks` table.
        ///
        /// Its SQL type is `Int4`.
        ///
        /// (Automatically generated by Diesel.)
        groupsize -> Int4,
        /// The `uri` column of the `tasks` table.
        ///
        /// Its SQL type is `Text`.
        ///
        /// (Automatically generated by Diesel.)
        uri -> Text,
    }
}

joinable!(infos -> tasks (task_id));

allow_tables_to_appear_in_same_query!(
    infos,
    tasks,
);
