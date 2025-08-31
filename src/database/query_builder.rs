//! SQL query builder for type-safe database operations
//!
//! Provides a fluent interface for building complex SQL queries with
//! compile-time safety and parameter validation.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Write as FmtWrite;

/// SQL query builder with type safety
#[derive(Debug, Clone)]
pub struct QueryBuilder {
    query_type: QueryType,
    table: String,
    columns: Vec<String>,
    values: Vec<QueryValue>,
    conditions: Vec<WhereCondition>,
    joins: Vec<Join>,
    order_by: Vec<OrderBy>,
    group_by: Vec<String>,
    having: Vec<WhereCondition>,
    limit: Option<usize>,
    offset: Option<usize>,
    parameters: HashMap<String, QueryValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum QueryType {
    Select,
    Insert,
    Update,
    Delete,
    CreateTable,
    CreateIndex,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QueryValue {
    Text(String),
    Integer(i64),
    Real(f64),
    Blob(Vec<u8>),
    Null,
    Boolean(bool),
}

impl QueryValue {
    /// Convert to rusqlite parameter value
    pub fn to_rusqlite_value(&self) -> rusqlite::types::Value {
        match self {
            QueryValue::Text(s) => rusqlite::types::Value::Text(s.clone()),
            QueryValue::Integer(i) => rusqlite::types::Value::Integer(*i),
            QueryValue::Real(f) => rusqlite::types::Value::Real(*f),
            QueryValue::Blob(b) => rusqlite::types::Value::Blob(b.clone()),
            QueryValue::Null => rusqlite::types::Value::Null,
            QueryValue::Boolean(b) => rusqlite::types::Value::Integer(if *b { 1 } else { 0 }),
        }
    }
}

impl From<&str> for QueryValue {
    fn from(s: &str) -> Self {
        QueryValue::Text(s.to_string())
    }
}

impl From<String> for QueryValue {
    fn from(s: String) -> Self {
        QueryValue::Text(s)
    }
}

impl From<i64> for QueryValue {
    fn from(i: i64) -> Self {
        QueryValue::Integer(i)
    }
}

impl From<i32> for QueryValue {
    fn from(i: i32) -> Self {
        QueryValue::Integer(i as i64)
    }
}

impl From<f64> for QueryValue {
    fn from(f: f64) -> Self {
        QueryValue::Real(f)
    }
}

impl From<bool> for QueryValue {
    fn from(b: bool) -> Self {
        QueryValue::Boolean(b)
    }
}

impl From<Vec<u8>> for QueryValue {
    fn from(b: Vec<u8>) -> Self {
        QueryValue::Blob(b)
    }
}

#[derive(Debug, Clone)]
pub struct WhereCondition {
    pub column: String,
    pub operator: ComparisonOperator,
    pub value: QueryValue,
    pub logical: LogicalOperator,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComparisonOperator {
    Equal,
    NotEqual,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    Like,
    NotLike,
    In,
    NotIn,
    IsNull,
    IsNotNull,
    Between,
    NotBetween,
}

impl ComparisonOperator {
    fn to_sql(&self) -> &'static str {
        match self {
            ComparisonOperator::Equal => "=",
            ComparisonOperator::NotEqual => "!=",
            ComparisonOperator::GreaterThan => ">",
            ComparisonOperator::GreaterThanOrEqual => ">=",
            ComparisonOperator::LessThan => "<",
            ComparisonOperator::LessThanOrEqual => "<=",
            ComparisonOperator::Like => "LIKE",
            ComparisonOperator::NotLike => "NOT LIKE",
            ComparisonOperator::In => "IN",
            ComparisonOperator::NotIn => "NOT IN",
            ComparisonOperator::IsNull => "IS NULL",
            ComparisonOperator::IsNotNull => "IS NOT NULL",
            ComparisonOperator::Between => "BETWEEN",
            ComparisonOperator::NotBetween => "NOT BETWEEN",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LogicalOperator {
    And,
    Or,
}

impl LogicalOperator {
    fn to_sql(&self) -> &'static str {
        match self {
            LogicalOperator::And => "AND",
            LogicalOperator::Or => "OR",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Join {
    pub join_type: JoinType,
    pub table: String,
    pub on_condition: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JoinType {
    Inner,
    Left,
    Right,
    Full,
    Cross,
}

impl JoinType {
    fn to_sql(&self) -> &'static str {
        match self {
            JoinType::Inner => "INNER JOIN",
            JoinType::Left => "LEFT JOIN",
            JoinType::Right => "RIGHT JOIN",
            JoinType::Full => "FULL OUTER JOIN",
            JoinType::Cross => "CROSS JOIN",
        }
    }
}

#[derive(Debug, Clone)]
pub struct OrderBy {
    pub column: String,
    pub direction: SortDirection,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SortDirection {
    Asc,
    Desc,
}

impl SortDirection {
    fn to_sql(&self) -> &'static str {
        match self {
            SortDirection::Asc => "ASC",
            SortDirection::Desc => "DESC",
        }
    }
}

/// Compiled query with SQL and parameters
#[derive(Debug, Clone)]
pub struct CompiledQuery {
    pub sql: String,
    pub parameters: Vec<QueryValue>,
    pub parameter_names: Vec<String>,
}

impl QueryBuilder {
    /// Create a new SELECT query builder
    pub fn select() -> Self {
        Self {
            query_type: QueryType::Select,
            table: String::new(),
            columns: Vec::new(),
            values: Vec::new(),
            conditions: Vec::new(),
            joins: Vec::new(),
            order_by: Vec::new(),
            group_by: Vec::new(),
            having: Vec::new(),
            limit: None,
            offset: None,
            parameters: HashMap::new(),
        }
    }

    /// Create a new INSERT query builder
    pub fn insert() -> Self {
        Self {
            query_type: QueryType::Insert,
            ..Self::select()
        }
    }

    /// Create a new UPDATE query builder
    pub fn update() -> Self {
        Self {
            query_type: QueryType::Update,
            ..Self::select()
        }
    }

    /// Create a new DELETE query builder
    pub fn delete() -> Self {
        Self {
            query_type: QueryType::Delete,
            ..Self::select()
        }
    }

    /// Set the target table
    pub fn from(mut self, table: &str) -> Self {
        self.table = table.to_string();
        self
    }

    /// Set the target table (alias for from)
    pub fn table(self, table: &str) -> Self {
        self.from(table)
    }

    /// Select specific columns
    pub fn columns(mut self, columns: &[&str]) -> Self {
        self.columns = columns.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Select all columns
    pub fn all_columns(mut self) -> Self {
        self.columns = vec!["*".to_string()];
        self
    }

    /// Add a WHERE condition
    pub fn where_eq<T: Into<QueryValue>>(mut self, column: &str, value: T) -> Self {
        self.conditions.push(WhereCondition {
            column: column.to_string(),
            operator: ComparisonOperator::Equal,
            value: value.into(),
            logical: LogicalOperator::And,
        });
        self
    }

    /// Add a WHERE condition with custom operator
    pub fn where_op<T: Into<QueryValue>>(
        mut self,
        column: &str,
        operator: ComparisonOperator,
        value: T,
    ) -> Self {
        self.conditions.push(WhereCondition {
            column: column.to_string(),
            operator,
            value: value.into(),
            logical: LogicalOperator::And,
        });
        self
    }

    /// Add an OR WHERE condition
    pub fn or_where<T: Into<QueryValue>>(mut self, column: &str, value: T) -> Self {
        let mut condition = WhereCondition {
            column: column.to_string(),
            operator: ComparisonOperator::Equal,
            value: value.into(),
            logical: LogicalOperator::Or,
        };

        if self.conditions.is_empty() {
            condition.logical = LogicalOperator::And;
        }

        self.conditions.push(condition);
        self
    }

    /// Add a WHERE IN condition
    pub fn where_in<T: Into<QueryValue>>(mut self, column: &str, values: Vec<T>) -> Self {
        // For IN clauses, we'll store as a JSON array for simplicity
        let json_values =
            serde_json::to_string(&values.into_iter().map(|v| v.into()).collect::<Vec<_>>())
                .unwrap_or_else(|_| "[]".to_string());

        self.conditions.push(WhereCondition {
            column: column.to_string(),
            operator: ComparisonOperator::In,
            value: QueryValue::Text(json_values),
            logical: LogicalOperator::And,
        });
        self
    }

    /// Add a LIKE condition
    pub fn where_like(mut self, column: &str, pattern: &str) -> Self {
        self.conditions.push(WhereCondition {
            column: column.to_string(),
            operator: ComparisonOperator::Like,
            value: QueryValue::Text(pattern.to_string()),
            logical: LogicalOperator::And,
        });
        self
    }

    /// Add a JOIN
    pub fn join(mut self, table: &str, on_condition: &str) -> Self {
        self.joins.push(Join {
            join_type: JoinType::Inner,
            table: table.to_string(),
            on_condition: on_condition.to_string(),
        });
        self
    }

    /// Add a LEFT JOIN
    pub fn left_join(mut self, table: &str, on_condition: &str) -> Self {
        self.joins.push(Join {
            join_type: JoinType::Left,
            table: table.to_string(),
            on_condition: on_condition.to_string(),
        });
        self
    }

    /// Add ORDER BY
    pub fn order_by(mut self, column: &str, direction: SortDirection) -> Self {
        self.order_by.push(OrderBy {
            column: column.to_string(),
            direction,
        });
        self
    }

    /// Add ORDER BY ASC
    pub fn order_by_asc(self, column: &str) -> Self {
        self.order_by(column, SortDirection::Asc)
    }

    /// Add ORDER BY DESC
    pub fn order_by_desc(self, column: &str) -> Self {
        self.order_by(column, SortDirection::Desc)
    }

    /// Add GROUP BY
    pub fn group_by(mut self, columns: &[&str]) -> Self {
        self.group_by = columns.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Set LIMIT
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set OFFSET
    pub fn offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Set values for INSERT/UPDATE
    pub fn values(mut self, values: HashMap<&str, QueryValue>) -> Self {
        self.columns = values.keys().map(|k| k.to_string()).collect();
        self.values = values.into_values().collect();
        self
    }

    /// Set a single value
    pub fn set<T: Into<QueryValue>>(mut self, column: &str, value: T) -> Self {
        if let Some(pos) = self.columns.iter().position(|c| c == column) {
            self.values[pos] = value.into();
        } else {
            self.columns.push(column.to_string());
            self.values.push(value.into());
        }
        self
    }

    /// Build the final SQL query
    pub fn build(self) -> Result<CompiledQuery> {
        let mut sql = String::new();
        let mut parameters = Vec::new();
        let mut parameter_names = Vec::new();

        match self.query_type {
            QueryType::Select => {
                write!(sql, "SELECT ")?;

                if self.columns.is_empty() {
                    write!(sql, "*")?;
                } else {
                    write!(sql, "{}", self.columns.join(", "))?;
                }

                if !self.table.is_empty() {
                    write!(sql, " FROM {}", self.table)?;
                }

                // Add JOINs
                for join in &self.joins {
                    write!(
                        sql,
                        " {} {} ON {}",
                        join.join_type.to_sql(),
                        join.table,
                        join.on_condition
                    )?;
                }

                // Add WHERE conditions
                self.build_where_clause(&mut sql, &mut parameters, &mut parameter_names)?;

                // Add GROUP BY
                if !self.group_by.is_empty() {
                    write!(sql, " GROUP BY {}", self.group_by.join(", "))?;
                }

                // Add ORDER BY
                if !self.order_by.is_empty() {
                    write!(sql, " ORDER BY ")?;
                    let order_clauses: Vec<String> = self
                        .order_by
                        .iter()
                        .map(|o| format!("{} {}", o.column, o.direction.to_sql()))
                        .collect();
                    write!(sql, "{}", order_clauses.join(", "))?;
                }

                // Add LIMIT and OFFSET
                if let Some(limit) = self.limit {
                    write!(sql, " LIMIT {}", limit)?;
                }
                if let Some(offset) = self.offset {
                    write!(sql, " OFFSET {}", offset)?;
                }
            }

            QueryType::Insert => {
                if self.table.is_empty() {
                    return Err(Error::Query("INSERT requires a table name".into()));
                }

                write!(sql, "INSERT INTO {} ", self.table)?;

                if !self.columns.is_empty() {
                    write!(sql, "({})", self.columns.join(", "))?;
                    write!(sql, " VALUES (")?;

                    for (i, value) in self.values.iter().enumerate() {
                        if i > 0 {
                            write!(sql, ", ")?;
                        }
                        write!(sql, "?")?;
                        parameters.push(value.clone());
                        parameter_names.push(format!("param_{}", i));
                    }

                    write!(sql, ")")?;
                }
            }

            QueryType::Update => {
                if self.table.is_empty() {
                    return Err(Error::Query("UPDATE requires a table name".into()));
                }

                write!(sql, "UPDATE {} SET ", self.table)?;

                for (i, (column, value)) in self.columns.iter().zip(self.values.iter()).enumerate()
                {
                    if i > 0 {
                        write!(sql, ", ")?;
                    }
                    write!(sql, "{} = ?", column)?;
                    parameters.push(value.clone());
                    parameter_names.push(format!("set_{}", i));
                }

                self.build_where_clause(&mut sql, &mut parameters, &mut parameter_names)?;
            }

            QueryType::Delete => {
                if self.table.is_empty() {
                    return Err(Error::Query("DELETE requires a table name".into()));
                }

                write!(sql, "DELETE FROM {}", self.table)?;
                self.build_where_clause(&mut sql, &mut parameters, &mut parameter_names)?;
            }

            _ => {
                return Err(Error::Query("Query type not yet implemented".into()));
            }
        }

        Ok(CompiledQuery {
            sql,
            parameters,
            parameter_names,
        })
    }

    /// Build WHERE clause
    fn build_where_clause(
        &self,
        sql: &mut String,
        parameters: &mut Vec<QueryValue>,
        parameter_names: &mut Vec<String>,
    ) -> Result<()> {
        if !self.conditions.is_empty() {
            write!(sql, " WHERE ")?;

            for (i, condition) in self.conditions.iter().enumerate() {
                if i > 0 {
                    write!(sql, " {} ", condition.logical.to_sql())?;
                }

                write!(sql, "{} {}", condition.column, condition.operator.to_sql())?;

                match &condition.operator {
                    ComparisonOperator::IsNull | ComparisonOperator::IsNotNull => {
                        // No parameter needed
                    }
                    ComparisonOperator::In | ComparisonOperator::NotIn => {
                        // Special handling for IN clauses
                        if let QueryValue::Text(json_str) = &condition.value {
                            if let Ok(values) = serde_json::from_str::<Vec<QueryValue>>(json_str) {
                                write!(sql, " (")?;
                                for (j, value) in values.iter().enumerate() {
                                    if j > 0 {
                                        write!(sql, ", ")?;
                                    }
                                    write!(sql, "?")?;
                                    parameters.push(value.clone());
                                    parameter_names.push(format!("in_param_{}_{}", i, j));
                                }
                                write!(sql, ")")?;
                            }
                        }
                    }
                    _ => {
                        write!(sql, " ?")?;
                        parameters.push(condition.value.clone());
                        parameter_names.push(format!("where_{}", i));
                    }
                }
            }
        }
        Ok(())
    }
}

impl std::fmt::Write for QueryBuilder {
    fn write_str(&mut self, _s: &str) -> std::fmt::Result {
        Ok(())
    }
}

/// Specialized query builders for common operations
pub struct UserQueries;

impl UserQueries {
    /// Find user by ID
    pub fn find_by_id(id: &str) -> Result<CompiledQuery> {
        QueryBuilder::select()
            .all_columns()
            .from("users")
            .where_eq("id", id)
            .build()
    }

    /// Find users by reputation range
    pub fn find_by_reputation_range(min_rep: f64, max_rep: f64) -> Result<CompiledQuery> {
        QueryBuilder::select()
            .all_columns()
            .from("users")
            .where_op(
                "reputation",
                ComparisonOperator::GreaterThanOrEqual,
                min_rep,
            )
            .where_op("reputation", ComparisonOperator::LessThanOrEqual, max_rep)
            .order_by_desc("reputation")
            .build()
    }

    /// Search users by username pattern
    pub fn search_by_username(pattern: &str) -> Result<CompiledQuery> {
        let search_pattern = format!("%{}%", pattern);
        QueryBuilder::select()
            .all_columns()
            .from("users")
            .where_like("username", &search_pattern)
            .where_eq("is_active", true)
            .order_by_asc("username")
            .build()
    }
}

pub struct GameQueries;

impl GameQueries {
    /// Find active games
    pub fn find_active_games(limit: usize) -> Result<CompiledQuery> {
        QueryBuilder::select()
            .all_columns()
            .from("games")
            .where_eq("state", "playing")
            .order_by_desc("created_at")
            .limit(limit)
            .build()
    }

    /// Find games with statistics
    pub fn find_with_stats(game_id: &str) -> Result<CompiledQuery> {
        QueryBuilder::select()
            .columns(&[
                "g.id",
                "g.state",
                "g.pot_size",
                "g.phase",
                "gs.total_bets",
                "gs.total_wagered",
                "gs.player_count",
            ])
            .from("games g")
            .left_join("game_statistics gs", "g.id = gs.game_id")
            .where_eq("g.id", game_id)
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_query_builder() {
        let query = QueryBuilder::select()
            .columns(&["id", "username", "reputation"])
            .from("users")
            .where_eq("active", true)
            .where_op("reputation", ComparisonOperator::GreaterThan, 50.0)
            .order_by_desc("reputation")
            .limit(10)
            .build()
            .unwrap();

        assert!(query.sql.contains("SELECT id, username, reputation"));
        assert!(query.sql.contains("FROM users"));
        assert!(query.sql.contains("WHERE"));
        assert!(query.sql.contains("ORDER BY reputation DESC"));
        assert!(query.sql.contains("LIMIT 10"));
        assert_eq!(query.parameters.len(), 2);
    }

    #[test]
    fn test_insert_query_builder() {
        let mut values = HashMap::new();
        values.insert("id", QueryValue::Text("user123".to_string()));
        values.insert("username", QueryValue::Text("alice".to_string()));
        values.insert("reputation", QueryValue::Real(75.5));

        let query = QueryBuilder::insert()
            .table("users")
            .values(values)
            .build()
            .unwrap();

        assert!(query.sql.contains("INSERT INTO users"));
        assert!(query.sql.contains("VALUES"));
        assert_eq!(query.parameters.len(), 3);
    }

    #[test]
    fn test_update_query_builder() {
        let query = QueryBuilder::update()
            .table("users")
            .set("reputation", 80.0)
            .set("updated_at", 1234567890i64)
            .where_eq("id", "user123")
            .build()
            .unwrap();

        assert!(query.sql.contains("UPDATE users SET"));
        assert!(query.sql.contains("WHERE"));
        assert_eq!(query.parameters.len(), 3);
    }

    #[test]
    fn test_user_queries() {
        let query = UserQueries::find_by_id("user123").expect("Valid query");
        assert!(query.sql.contains("SELECT * FROM users WHERE id = ?"));
        assert_eq!(query.parameters.len(), 1);

        let query = UserQueries::search_by_username("alice").expect("Valid query");
        assert!(query.sql.contains("LIKE"));
        assert!(query.sql.contains("is_active"));
    }
}
