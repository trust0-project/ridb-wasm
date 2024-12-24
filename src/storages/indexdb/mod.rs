use js_sys::{Array, Object, Promise, Reflect};
use serde_wasm_bindgen::to_value;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen::prelude::{wasm_bindgen, Closure};
use wasm_bindgen_futures::JsFuture;
use crate::logger::Logger;
use crate::query::Query;
use crate::storage::internals::base_storage::BaseStorage;
use crate::storage::internals::core::CoreStorage;
use crate::operation::{OpType, Operation};
use web_sys::{IdbDatabase, IdbOpenDbRequest, IdbRequest};
use std::sync::Arc;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Weak;
use lazy_static::lazy_static;

use super::base::Storage;

#[wasm_bindgen(typescript_custom_section)]
const TS_APPEND_CONTENT: &'static str = r#"
/**
 * Represents an IndexDB storage system extending the base storage functionality.
 *
 * @template T - The schema type.
 */
export class IndexDB<T extends SchemaTypeRecord> extends BaseStorage<T> {
    /**
     * Frees the resources used by the in-memory storage.
     */
    free(): void;

    static create<SchemasCreate extends SchemaTypeRecord>(
        dbName: string,
        schemas: SchemasCreate,
    ): Promise<
        IndexDB<
            SchemasCreate
        >
    >;
}
"#;

#[wasm_bindgen(skip_typescript)]
pub struct IndexDB {
    core: CoreStorage,
    base: BaseStorage,
    db: IdbDatabase,
    _error_handler: Option<Closure<dyn FnMut(web_sys::Event)>>,
    _success_handler: Option<Closure<dyn FnMut(web_sys::Event)>>,
} 

impl Drop for IndexDB {
    fn drop(&mut self) {
        self._error_handler.take();
        self._success_handler.take();
        self.db.close();
    }
}

async fn idb_request_result(request: IdbRequest) -> Result<JsValue, JsValue> {
    let promise = Promise::new(&mut |resolve, reject| {

        let reject2 = reject.clone();
        let success_callback = Closure::once(Box::new(move |event: web_sys::Event| {
            let request: IdbRequest = event.target()
                .unwrap()
                .dyn_into()
                .unwrap();
            
            match request.result() {
                Ok(result) => resolve.call1(&JsValue::undefined(), &result).unwrap(),
                Err(e) => reject.call1(&JsValue::undefined(), &e).unwrap(),
            }
        }));

        let error_callback = Closure::once(Box::new(move |event: web_sys::Event| {
            let request: IdbRequest = event.target()
                .unwrap()
                .dyn_into()
                .unwrap();
            
            let error = request.error().unwrap();
            reject2.call1(&JsValue::undefined(), &error.unwrap()).unwrap();
        }));

        request.set_onsuccess(Some(success_callback.as_ref().unchecked_ref()));
        request.set_onerror(Some(error_callback.as_ref().unchecked_ref()));

        // The closures will automatically be dropped after the Promise resolves/rejects
        success_callback.forget();
        error_callback.forget();
    });

    JsFuture::from(promise).await
}

impl Storage for IndexDB {
    async fn write(&self, op: &Operation) -> Result<JsValue, JsValue> {
        let store_name = &op.collection;
        
        let store_names = self.db.object_store_names();
        let stores: Vec<String> = (0..store_names.length())
            .filter_map(|i| {
                let store = store_names.get(i)?;
                Some(store.as_str().to_string())
            })
            .collect();
        Logger::debug(&JsValue::from_str(&format!(
            "Available stores: {:?}, Attempting to access: {}",
            stores, store_name
        )));

        let transaction = match self.db.transaction_with_str_and_mode(
            store_name,
            web_sys::IdbTransactionMode::Readwrite,
        ) {
            Ok(t) => t,
            Err(e) => {
                web_sys::console::error_1(&JsValue::from_str(&format!(
                    "Failed to create transaction for store '{}': {:?}",
                    store_name, e
                )));
                return Err(JsValue::from_str(&format!(
                    "Failed to access store '{}'. Available stores: {:?}",
                    store_name, stores
                )));
            }
        };

        let store = match transaction.object_store(store_name) {
            Ok(s) => s,
            Err(e) => return Err(e),
        };

        let schema = self.base.schemas.get(op.collection.as_str()).ok_or_else(|| JsValue::from_str("Collection not found"))?;

        match op.op_type {
            OpType::CREATE | OpType::UPDATE => {
                let document = op.data.clone();
                
                // Extract primary key
                let primary_key = schema.primary_key.clone();
                let pk_value = match Reflect::get(&document, &JsValue::from_str(&primary_key)) {
                    Ok(v) => v,
                    Err(e) => return Err(e),
                };

                if pk_value.is_undefined() || pk_value.is_null() {
                    return Err(JsValue::from_str("Document must contain a primary key"));
                }

                // Validate document against schema
                schema.validate_schema(document.clone())?;

                // Store the document and wait for completion
                let request = store.put_with_key(&document, &pk_value)?;
                idb_request_result(request).await?;

                Ok(document.clone())
            },
            OpType::DELETE => {
                let pk_value = op.data.clone();
                if pk_value.is_undefined() || pk_value.is_null() {
                    return Err(JsValue::from_str("Primary key value is required for delete operation"));
                }

                // Delete the document and wait for completion
                let request = store.delete(&pk_value)?;
                let promise = Promise::new(&mut |resolve, reject| {
                    let onsucess = Closure::once(Box::new(move |_event: web_sys::Event| {
                        resolve.call1(&JsValue::undefined(), &JsValue::from_str("Document deleted")).unwrap();
                    }));
                    
                    let onerror = Closure::once(Box::new(move |e: web_sys::Event| {
                        reject.call1(&JsValue::undefined(), &e).unwrap();
                    }));
                    
                    request.set_onsuccess(Some(onsucess.as_ref().unchecked_ref()));
                    request.set_onerror(Some(onerror.as_ref().unchecked_ref()));
                    onsucess.forget();
                    onerror.forget();
                });

                JsFuture::from(promise).await
            },
            _ => Err(JsValue::from_str("Unsupported operation type")),
        }
    }

    async fn find(&self, collection_name: &str, query: Query) -> Result<JsValue, JsValue> {
        let store_name = collection_name;
        
        let store_names = self.db.object_store_names();
        let stores: Vec<String> = (0..store_names.length())
        .filter_map(|i| {
            let store = store_names.get(i)?;
            Some(store.as_str().to_string())
        })
        .collect();

        let transaction = match self.db.transaction_with_str(store_name) {
            Ok(t) => t,
            Err(e) => {
                web_sys::console::error_1(&JsValue::from_str(&format!(
                    "Failed to create transaction for store '{}': {:?}",
                    store_name, e
                )));
                return Err(JsValue::from_str(&format!(
                    "Failed to access store '{}'. Available stores: {:?}",
                    store_name, stores
                )));
            }
        };

        let store = transaction.object_store(store_name)?;
        
        let normalized_query = query.parse()?;
        let request = store.get_all()?;
        let normalized_query = normalized_query.clone();
        let promise = Promise::new(&mut |resolve, _reject| {
            let value = normalized_query.clone();
            let core = self.core.clone();
            let onsucess = Closure::once(Box::new(move |event: web_sys::Event| {
                let request: IdbRequest = event.target().unwrap().dyn_into().unwrap();
                let result = request.result().unwrap();
                // Filter documents based on query
                let filtered = Array::new();


                if !result.is_undefined() && !result.is_null() {
                    let documents = Array::from(&result);

                    for i in 0..documents.length() {
                        let doc = documents.get(i);
                        if let Ok(matches) = core.document_matches_query(&doc, &value) {
                            if matches {
                                filtered.push(&doc);
                            }
                        }
                    }
                }
                
                resolve.call1(&JsValue::undefined(), &filtered).unwrap();
            }));
            
            request.set_onsuccess(Some(onsucess.as_ref().unchecked_ref()));
            onsucess.forget();
        });

        JsFuture::from(promise).await
    }

    async fn find_document_by_id(&self, collection_name: &str, primary_key_value: JsValue) -> Result<JsValue, JsValue> {
        let store_name = collection_name;
        let transaction = self.db.transaction_with_str(store_name)?;
        let store = transaction.object_store(store_name)?;
        
        let request = store.get(&primary_key_value)?;
        
        let promise = Promise::new(&mut |resolve, reject| {
            let onsucess = Closure::once(Box::new(move |event: web_sys::Event| {
                let request: IdbRequest = event.target().unwrap().dyn_into().unwrap();
                let result = request.result().unwrap();
                
                if result.is_undefined() {
                    reject.call1(&JsValue::undefined(), &JsValue::from_str("Document not found")).unwrap();
                } else {
                    resolve.call1(&JsValue::undefined(), &result).unwrap();
                }
            }));
            
            request.set_onsuccess(Some(onsucess.as_ref().unchecked_ref()));
            onsucess.forget();
        });

        JsFuture::from(promise).await
    }

    async fn count(&self,collection_name: &str,   query: Query) -> Result<JsValue, JsValue> {
        let store_name = collection_name;
        let transaction = self.db.transaction_with_str(store_name)?;
        let store = transaction.object_store(store_name)?;
        
        let normalized_query = query.parse()?;
        let request = store.get_all()?;
        let normalized_query = normalized_query.clone();        
        let promise = Promise::new(&mut |resolve, _reject| {
            let value = normalized_query.clone();
            let core = self.core.clone();
            let onsucess = Closure::once(Box::new(move |event: web_sys::Event| {
                let request: IdbRequest = event.target().unwrap().dyn_into().unwrap();
                let result = request.result().unwrap();
                let documents = Array::from(&result);
                
                let mut count = 0;
                for i in 0..documents.length() {
                    let doc = documents.get(i);
                    if let Ok(matches) = core.document_matches_query(&doc, &value) {
                        if matches {
                            count += 1;
                        }
                    }
                }
                
                resolve.call1(&JsValue::undefined(), &JsValue::from_f64(count as f64)).unwrap();
            }));
            
            request.set_onsuccess(Some(onsucess.as_ref().unchecked_ref()));
            onsucess.forget();
        });

        JsFuture::from(promise).await
    }

    async fn close(&mut self) -> Result<JsValue, JsValue> {
        // Wait for any pending transactions to complete
        let store_names = self.db.object_store_names();
        let stores: Vec<String> = (0..store_names.length())
            .filter_map(|i| {
                let store = store_names.get(i)?;
                Some(store.as_str().to_string())
            })
            .collect();

        // Create a read transaction for each store to ensure all operations are complete
        for store_name in stores {
            let transaction = self.db.transaction_with_str(&store_name)?;
            
            // Wait for the transaction to complete
            let promise = Promise::new(&mut |resolve, reject| {
                let oncomplete = Closure::once(Box::new(move |_: web_sys::Event| {
                    resolve.call0(&JsValue::undefined()).unwrap();
                }));
                
                let onerror = Closure::once(Box::new(move |e| {
                    reject.call1(&JsValue::undefined(), &e).unwrap();
                }));
                
                transaction.set_oncomplete(Some(oncomplete.as_ref().unchecked_ref()));
                transaction.set_onerror(Some(onerror.as_ref().unchecked_ref()));
                
                oncomplete.forget();
                onerror.forget();
            });
            
            JsFuture::from(promise).await?;
        }

        // Remove the connection from the pool
        POOL.remove_connection(&self.base.name);

        // Now safe to close the database
        self.db.close();
        Ok(JsValue::from_str("IndexDB database closed"))
    }

    async fn start(&mut self) -> Result<JsValue, JsValue> {
        // Check if database is closed by attempting a simple transaction
        let test_store = self.db.object_store_names().get(0);
        if test_store.is_some() {
            let store_name = test_store.unwrap();
            if let Err(_) = self.db.transaction_with_str(&store_name) {
                // Database is closed, need to reopen
                let schemas_js = Object::new();
                for (collection, schema) in self.base.schemas.iter() {
                    let _ = Reflect::set(&schemas_js, &JsValue::from_str(collection), &to_value(&schema)?);
                }
                let db = create_database(&self.base.name, &schemas_js).await?;
                // Update the pool with new connection
                POOL.store_connection(self.base.name.clone(), Arc::downgrade(&db));
                self.db = (*db).clone();
            }
            Ok(JsValue::from_str("IndexDB database started"))
        } else{
            Ok(JsValue::from_str("IndexDB database already started"))

        }
        
    }
}

async fn create_database(name: &str, schemas_js: &Object) -> Result<Arc<IdbDatabase>, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("No window object"))?;
    let idb = window.indexed_db()?.ok_or_else(|| JsValue::from_str("IndexedDB not available"))?;
    
    let version = 1;
    let db_request = idb.open_with_u32(name, version)?;

    // Clone keys before entering the Promise
    let keys_array = Object::keys(schemas_js);
    let keys_vec: Vec<String> = (0..keys_array.length())
        .filter_map(|i| keys_array.get(i).as_string())
        .collect();
    
    let db = JsFuture::from(Promise::new(&mut |resolve, reject| {
        let keys = keys_vec.clone();
        let onupgradeneeded = Closure::once(Box::new(move |event: web_sys::Event| {
            let db: IdbDatabase = event.target()
                .unwrap()
                .dyn_into::<IdbOpenDbRequest>()
                .unwrap()
                .result()
                .unwrap()
                .dyn_into()
                .unwrap();
            
            for collection_name in keys {
                if !db.object_store_names().contains(&collection_name) {
                    db.create_object_store(&collection_name)
                        .expect("Failed to create object store");
                }
            }
        }));

        let onsuccess = Closure::once(Box::new(move |event: web_sys::Event| {
            let db: IdbDatabase = event.target()
                .unwrap()
                .dyn_into::<IdbOpenDbRequest>()
                .unwrap()
                .result()
                .unwrap()
                .dyn_into()
                .unwrap();
            resolve.call1(&JsValue::undefined(), &db).unwrap();
        }));

        let onerror = Closure::once(Box::new(move |e: web_sys::Event| {
            reject.call1(&JsValue::undefined(), &e).unwrap();
        }));

        db_request.set_onupgradeneeded(Some(onupgradeneeded.as_ref().unchecked_ref()));
        db_request.set_onsuccess(Some(onsuccess.as_ref().unchecked_ref()));
        db_request.set_onerror(Some(onerror.as_ref().unchecked_ref()));

        onupgradeneeded.forget();
        onsuccess.forget();
        onerror.forget();
    })).await?;

    Ok(Arc::new(db.dyn_into::<IdbDatabase>()?))
}

#[wasm_bindgen]
impl IndexDB {
    #[wasm_bindgen]
    pub async fn create(name: &str, schemas_js: Object) -> Result<IndexDB, JsValue> {
        let base = BaseStorage::new(
            name.to_string(),
            schemas_js.clone(),
            None
        )?;

        // Try to get existing connection from pool
        let db = match POOL.get_connection(name) {
            Some(db) => db,
            None => {
                // Create new connection if none exists
                let db = create_database(name, &schemas_js).await?;
                POOL.store_connection(name.to_string(), Arc::downgrade(&db));
                db
            }
        };

        Ok(IndexDB {
            base,
            core: CoreStorage {},
            db: (*db).clone(),
            _error_handler: None,
            _success_handler: None,
        })
    }

    #[wasm_bindgen(js_name = "write")]
    pub async fn write_js(&self, op: &Operation) -> Result<JsValue, JsValue> {
        self.write(op).await
    }

    #[wasm_bindgen(js_name = "find")]
    pub async fn find_js(&self, collection_name: &str, query: JsValue) -> Result<JsValue, JsValue> {
        let schema = self.base.schemas.get(collection_name).ok_or_else(|| JsValue::from_str("Collection not found"))?;
        self.find(collection_name, Query::new(query, schema.clone())?).await
    }

    #[wasm_bindgen(js_name = "findDocumentById")]
    pub async fn find_document_by_id_js(&self, collection_name: &str, primary_key: JsValue) -> Result<JsValue, JsValue> {
        self.find_document_by_id(collection_name, primary_key).await
    }

    #[wasm_bindgen(js_name = "count")]
    pub async fn count_js(&self, collection_name: &str, query: JsValue) -> Result<JsValue, JsValue> {
        let schema = self.base.schemas.get(collection_name).ok_or_else(|| JsValue::from_str("Collection not found"))?;
        self.count(collection_name, Query::new(query, schema.clone())?).await
    }

    #[wasm_bindgen(js_name = "close")]
    pub async fn close_js(&mut self) -> Result<JsValue, JsValue> {
        self.close().await
    }

    #[wasm_bindgen(js_name = "start")]
    pub async fn start_js(&mut self) -> Result<JsValue, JsValue> {
        self.start().await
    }
}
// Global connection pool
lazy_static! {
    static ref POOL: IndexDBPool = IndexDBPool::new();
}

// Add these trait implementations before the IndexDBPool struct
unsafe impl Send for IndexDBPool {}
unsafe impl Sync for IndexDBPool {}

pub struct IndexDBPool {
    connections: Arc<Mutex<HashMap<String, Arc<IdbDatabase>>>>,
}

impl IndexDBPool {
    fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new()))
        }
    }

    /// Retrieves a connection from the pool, recreating it if it's closed
    fn get_connection(&self, name: &str) -> Option<Arc<IdbDatabase>> {
        let mut connections = self.connections.lock();
        if let Some(db) = connections.get(name) {
            // Check if the database connection is still valid
            if db.is_closed() {
                // Remove the closed connection
                connections.remove(name);
                None
            } else {
                Some(db.clone())
            }
        } else {
            None
        }
    }

    /// Stores a new connection in the pool
    fn store_connection(&self, name: String, db: Weak<IdbDatabase>) {
        let mut connections = self.connections.lock();
        if let Some(arc_db) = db.upgrade() {
            connections.insert(name, arc_db);
        }
    }

    /// Removes a connection from the pool
    fn remove_connection(&self, name: &str) {
        let mut connections = self.connections.lock();
        connections.remove(name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use wasm_bindgen_test::*;
    
    // Configure tests to run in browser
    wasm_bindgen_test_configure!(run_in_browser);

    fn json_str_to_js_value(json_str: &str) -> Result<JsValue, JsValue> {
        let json_value: Value =
            serde_json::from_str(json_str).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(value_to_js_value(&json_value))
    }

    fn value_to_js_value(value: &Value) -> JsValue {
        match value {
            Value::Null => JsValue::null(),
            Value::Bool(b) => JsValue::from_bool(*b),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    JsValue::from_f64(i as f64)
                } else if let Some(f) = n.as_f64() {
                    JsValue::from_f64(f)
                } else {
                    JsValue::undefined()
                }
            }
            Value::String(s) => JsValue::from_str(s),
            Value::Array(arr) => {
                let js_array = Array::new();
                for item in arr {
                    js_array.push(&value_to_js_value(item));
                }
                js_array.into()
            }
            Value::Object(obj) => {
                let js_obj = Object::new();
                for (key, value) in obj {
                    js_sys::Reflect::set(
                        &js_obj,
                        &JsValue::from_str(key),
                        &value_to_js_value(value),
                    )
                        .unwrap();
                }
                js_obj.into()
            }
        }
    }

    #[wasm_bindgen_test(async)]
    async fn test_empty_indexdb_storage() {
        let schemas_obj = Object::new();
        let schema_str = "{ \"version\": 1, \"primaryKey\": \"id\", \"type\": \"object\", \"properties\": { \"id\": { \"type\": \"string\", \"maxLength\": 60 } } }";
        let schema = json_str_to_js_value(schema_str).unwrap();
        Reflect::set(&schemas_obj, &JsValue::from_str("demo"), &schema).unwrap();
        
        let db = IndexDB::create("test_db", schemas_obj).await;
        assert!(db.is_ok());
        
        // Clean up
        db.unwrap().close().await.unwrap();
    }

    #[wasm_bindgen_test(async)]
    async fn test_indexdb_storage_create_operation() {
        let schemas_obj = Object::new();
        let schema_str = r#"{
            "version": 1,
            "primaryKey": "id",
            "type": "object",
            "required": ["id", "name"],
            "properties": {
                "id": { "type": "string", "maxLength": 60 },
                "name": { "type": "string" }
            }
        }"#;
        let schema = json_str_to_js_value(schema_str).unwrap();
        Reflect::set(&schemas_obj, &JsValue::from_str("demo"), &schema).unwrap();
        
        let mut db = IndexDB::create("test_db_create", schemas_obj).await.unwrap();

        // Create a new item
        let new_item = Object::new();
        Reflect::set(&new_item, &JsValue::from_str("id"), &JsValue::from_str("1234")).unwrap();
        Reflect::set(&new_item, &JsValue::from_str("name"), &JsValue::from_str("Test Item")).unwrap();

        let op = Operation {
            collection: "demo".to_string(),
            op_type: OpType::CREATE,
            data: new_item.clone().into(),
            indexes: vec![],
        };

        // Test successful creation
        let created = db.write(&op).await.unwrap();
        assert_eq!(
            Reflect::get(&created, &JsValue::from_str("id")).unwrap(),
            JsValue::from_str("1234")
        );

        // Test document retrieval
        let found = db
            .find_document_by_id("demo", JsValue::from_str("1234"))
            .await
            .unwrap();
        assert_eq!(
            Reflect::get(&found, &JsValue::from_str("name")).unwrap(),
            JsValue::from_str("Test Item")
        );

        // Clean up
        db.close().await.unwrap();
    }

    #[wasm_bindgen_test(async)]
    async fn test_indexdb_storage_find() {
        let schemas_obj = Object::new();
        let schema_str = r#"{
            "version": 1,
            "primaryKey": "id",
            "type": "object",
            "properties": {
                "id": { "type": "string" },
                "name": { "type": "string" },
                "age": { "type": "number" },
                "status": { "type": "string" }
            }
        }"#;
        let schema = json_str_to_js_value(schema_str).unwrap();
        Reflect::set(&schemas_obj, &JsValue::from_str("demo"), &schema).unwrap();
        
        let mut db = IndexDB::create("test_db_find", schemas_obj).await.unwrap();

        // Create test documents
        let items = vec![
            json_str_to_js_value(r#"{
                "id": "1", "name": "Alice", "age": 30, "status": "active"
            }"#).unwrap(),
            json_str_to_js_value(r#"{
                "id": "2", "name": "Bob", "age": 25, "status": "inactive"
            }"#).unwrap(),
            json_str_to_js_value(r#"{
                "id": "3", "name": "Charlie", "age": 35, "status": "active"
            }"#).unwrap(),
        ];

        for item in items {
            let create_op = Operation {
                collection: "demo".to_string(),
                op_type: OpType::CREATE,
                data: item,
                indexes: vec![],
            };
            db.write(&create_op).await.unwrap();
        }

        // Test find with query
        let query_value = json_str_to_js_value(r#"{
            "status": "active",
            "age": { "$gt": 30 }
        }"#).unwrap();
        
        let result = db.find_js("demo", query_value).await.unwrap();
        let result_array = Array::from(&result);
        
        assert_eq!(result_array.length(), 1);
        let first_doc = result_array.get(0);
        assert_eq!(
            Reflect::get(&first_doc, &JsValue::from_str("name")).unwrap(),
            JsValue::from_str("Charlie")
        );

        // Clean up
        db.close().await.unwrap();
    }

    #[wasm_bindgen_test(async)]
    async fn test_indexdb_storage_count() {
        let schemas_obj = Object::new();
        let schema_str = r#"{
            "version": 1,
            "primaryKey": "id",
            "type": "object",
            "properties": {
                "id": { "type": "string" },
                "name": { "type": "string" },
                "status": { "type": "string" }
            }
        }"#;
        let schema = json_str_to_js_value(schema_str).unwrap();
        Reflect::set(&schemas_obj, &JsValue::from_str("demo"), &schema).unwrap();
        
        let mut db = IndexDB::create("test_db_count", schemas_obj).await.unwrap();

        // Create test documents
        let items = vec![
            json_str_to_js_value(r#"{
                "id": "1", "name": "Alice", "status": "active"
            }"#).unwrap(),
            json_str_to_js_value(r#"{
                "id": "2", "name": "Bob", "status": "inactive"
            }"#).unwrap(),
            json_str_to_js_value(r#"{
                "id": "3", "name": "Charlie", "status": "active"
            }"#).unwrap(),
        ];

        for item in items {
            let create_op = Operation {
                collection: "demo".to_string(),
                op_type: OpType::CREATE,
                data: item,
                indexes: vec![],
            };
            db.write(&create_op).await.unwrap();
        }

        // Test count with query
        let query_value = json_str_to_js_value(r#"{
            "status": "active"
        }"#).unwrap();
        
        let result = db.count_js("demo", query_value).await.unwrap();
        assert_eq!(result.as_f64().unwrap(), 2.0);

        // Clean up
        db.close().await.unwrap();
    }

    #[wasm_bindgen_test(async)]
    async fn test_indexdb_multiple_collections() {
        // Create schemas for two collections
        let schemas_obj = Object::new();
        
        // Schema for users collection
        let users_schema = json_str_to_js_value(r#"{
            "version": 1,
            "primaryKey": "id",
            "type": "object",
            "properties": {
                "id": { "type": "string" },
                "name": { "type": "string" },
                "email": { "type": "string" }
            }
        }"#).unwrap();
        
        // Schema for products collection
        let products_schema = json_str_to_js_value(r#"{
            "version": 1,
            "primaryKey": "id",
            "type": "object",
            "properties": {
                "id": { "type": "string" },
                "name": { "type": "string" },
                "price": { "type": "number" }
            }
        }"#).unwrap();
        
        Reflect::set(&schemas_obj, &JsValue::from_str("users"), &users_schema).unwrap();
        Reflect::set(&schemas_obj, &JsValue::from_str("products"), &products_schema).unwrap();
        
        let mut db = IndexDB::create("test_db_multiple_collections", schemas_obj).await.unwrap();

        // Insert data only into users collection
        let user = json_str_to_js_value(r#"{
            "id": "1",
            "name": "John Doe",
            "email": "john@example.com"
        }"#).unwrap();

        let create_op = Operation {
            collection: "users".to_string(),
            op_type: OpType::CREATE,
            data: user,
            indexes: vec![],
        };
        
        db.write(&create_op).await.unwrap();

        // Query the empty products collection
        let empty_query = json_str_to_js_value("{}").unwrap();
        
        // Find all products (should be empty)
        let products_result = db.find_js("products", empty_query.clone()).await.unwrap();
        let products_array = Array::from(&products_result);
        assert_eq!(products_array.length(), 0);

        // Count products (should be 0)
        let count_result = db.count_js("products", empty_query).await.unwrap();
        assert_eq!(count_result.as_f64().unwrap(), 0.0);

        // Clean up
        db.close().await.unwrap();
    }
}

// Add this extension trait
trait IdbDatabaseExt {
    fn is_closed(&self) -> bool;
}

impl IdbDatabaseExt for IdbDatabase {
    fn is_closed(&self) -> bool {
        // Attempt to start a dummy transaction to see if the database is closed
        match self.transaction_with_str("__non_existent_store__") {
            Ok(_) => false,
            Err(_) => true,
        }
    }
}

