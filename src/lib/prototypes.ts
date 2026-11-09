// One `search_prototypes` result. Mirrors `PrototypeMatch` in calligraphy.rs.
export interface PrototypeMatch {
  path: string
  blueprint: string
  display_name: string
  leaf: string
  class: string
  /** Where `display_name` came from. */
  name_source: 'custom' | 'game' | 'bundled' | 'path'
  /** Decimal string, to keep full u64 precision. */
  prototype_id: string
  /** Decimal string, to keep full u64 precision. */
  prototype_guid: string
}
