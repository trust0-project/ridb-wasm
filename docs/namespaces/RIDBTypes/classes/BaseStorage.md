[**@trust0/ridb**](../../../README.md) • **Docs**

***

[@trust0/ridb](../../../README.md) / [RIDBTypes](../README.md) / BaseStorage

# Class: BaseStorage\<T\>

Represents the base storage implementation, extending `StorageInternal`.

## Extends

- [`StorageInternal`](StorageInternal.md)\<`T`\>

## Extended by

- [`InMemory`](InMemory.md)

## Type Parameters

• **T** *extends* [`SchemaType`](../type-aliases/SchemaType.md)

The schema type.

## Constructors

### new BaseStorage()

> **new BaseStorage**\<`T`\>(`name`, `schema_type`, `migration`): [`BaseStorage`](BaseStorage.md)\<`T`\>

Creates a new `BaseStorage` instance with the provided name and schema type.

#### Parameters

• **name**: `string`

The name of the storage.

• **schema\_type**: `T`

The schema type of the storage.

• **migration**: `MigrationPathsForSchema`\<`T`\>

#### Returns

[`BaseStorage`](BaseStorage.md)\<`T`\>

#### Overrides

`StorageInternal<T>.constructor`

#### Defined in

pkg/ridb\_rust.d.ts:208

## Properties

### name

> `readonly` **name**: `string`

The name of the storage.

#### Defined in

pkg/ridb\_rust.d.ts:213

***

### schema

> `readonly` **schema**: [`Schema`](Schema.md)\<`T`\>

The schema associated with the storage.

#### Defined in

pkg/ridb\_rust.d.ts:218

## Methods

### close()

> **close**(): `Promise`\<`void`\>

Closes the storage.

#### Returns

`Promise`\<`void`\>

A promise that resolves when the storage is closed.

#### Overrides

[`StorageInternal`](StorageInternal.md).[`close`](StorageInternal.md#close)

#### Defined in

pkg/ridb\_rust.d.ts:225

***

### count()

> **count**(`query`): `Promise`\<`number`\>

Counts the number of documents in the storage.

#### Parameters

• **query**: `QueryType`\<`T`\>

#### Returns

`Promise`\<`number`\>

A promise that resolves to the number of documents.

#### Overrides

[`StorageInternal`](StorageInternal.md).[`count`](StorageInternal.md#count)

#### Defined in

pkg/ridb\_rust.d.ts:232

***

### find()

> **find**(`query`): `Promise`\<[`Doc`](../type-aliases/Doc.md)\<`T`\>[]\>

Queries the storage.

#### Parameters

• **query**: `QueryType`\<`T`\>

#### Returns

`Promise`\<[`Doc`](../type-aliases/Doc.md)\<`T`\>[]\>

A promise that resolves when the query is complete.

#### Overrides

[`StorageInternal`](StorageInternal.md).[`find`](StorageInternal.md#find)

#### Defined in

pkg/ridb\_rust.d.ts:247

***

### findDocumentById()

> **findDocumentById**(`id`): `Promise`\<`null` \| [`Doc`](../type-aliases/Doc.md)\<`T`\>\>

Finds a document by its ID.

#### Parameters

• **id**: `string`

The ID of the document to find.

#### Returns

`Promise`\<`null` \| [`Doc`](../type-aliases/Doc.md)\<`T`\>\>

A promise that resolves to the found document or null.

#### Overrides

[`StorageInternal`](StorageInternal.md).[`findDocumentById`](StorageInternal.md#finddocumentbyid)

#### Defined in

pkg/ridb\_rust.d.ts:240

***

### free()

> **free**(): `void`

Frees the resources used by the base storage.

#### Returns

`void`

#### Defined in

pkg/ridb\_rust.d.ts:199

***

### remove()

> **remove**(`id`): `Promise`\<`void`\>

Removes a document by its ID.

#### Parameters

• **id**: `string`

The ID of the document to remove.

#### Returns

`Promise`\<`void`\>

A promise that resolves when the document is removed.

#### Overrides

[`StorageInternal`](StorageInternal.md).[`remove`](StorageInternal.md#remove)

#### Defined in

pkg/ridb\_rust.d.ts:255

***

### write()

> **write**(`op`): `Promise`\<[`Doc`](../type-aliases/Doc.md)\<`T`\>\>

Writes an operation to the storage.

#### Parameters

• **op**: [`Operation`](../type-aliases/Operation.md)\<`T`\>

The operation to write.

#### Returns

`Promise`\<[`Doc`](../type-aliases/Doc.md)\<`T`\>\>

A promise that resolves to the document written.

#### Overrides

[`StorageInternal`](StorageInternal.md).[`write`](StorageInternal.md#write)

#### Defined in

pkg/ridb\_rust.d.ts:263
