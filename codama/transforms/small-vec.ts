import {
  arrayTypeNode,
  definedTypeLinkNode,
  numberTypeNode,
  prefixedCountNode,
  publicKeyTypeNode,
  type NumberFormat,
  type TypeNode,
} from "codama";

const SMALL_VEC_PATTERN = /^SmallVec<(u\d+),\s*(.+)>$/;

/**
 * The codama-renderers-rust dependency detector greedily scans rendered content
 * for any token shaped like `lowercase_word::Path` and treats the lowercase
 * prefix as a crate name.  Rust primitive types such as `i64`, `u128`, etc.
 * therefore trip the detector when they appear inside doc strings like
 * `` `i64::MAX` `` and produce spurious "missing dependency" errors.  We strip
 * the `::` from every Rust primitive path in IDL docs *before* parsing so the
 * rendered output never carries the offending substring.
 */
const RUST_PRIMITIVE_PATH = /\b([ui](?:8|16|32|64|128)|f32|f64|bool|usize|isize)::/g;

export type SmallVecBinding = {
  readonly sanitizedName: string;
  readonly prefix: NumberFormat;
  readonly innerTypeRef: string;
};

/**
 * Walks the raw Anchor IDL JSON and:
 *   1. Rewrites every `{ "defined": "SmallVec<N, T>" }` reference to
 *      `{ "defined": "<sanitized>" }`.
 *   2. Adds a placeholder struct entry for each sanitized name to `types[]`
 *      so codama's Anchor parser does not choke on a dangling link.
 *
 * The placeholder is later overwritten in the parsed AST by
 * `applySmallVecTypeOverrides` with a proper prefixed-count array node — see
 * `codama/codama.ts` for the pipeline ordering.
 */
export function rewriteSmallVecsInIdl(rawIdl: unknown): {
  idl: any;
  bindings: SmallVecBinding[];
} {
  const seen = new Map<string, SmallVecBinding>();

  const walk = (value: any, parentKey?: string): any => {
    if (Array.isArray(value)) {
      if (parentKey === "docs") {
        return value.map((entry) =>
          typeof entry === "string"
            ? entry.replace(RUST_PRIMITIVE_PATH, "$1·")
            : walk(entry)
        );
      }
      return value.map((v) => walk(v));
    }
    if (value === null || typeof value !== "object") return value;

    if (typeof value.defined === "string") {
      const match = value.defined.match(SMALL_VEC_PATTERN);
      if (match) {
        const [, prefix, inner] = match;
        const sanitized = sanitizedNameFor(prefix, inner);
        if (!seen.has(sanitized)) {
          seen.set(sanitized, {
            sanitizedName: sanitized,
            prefix: prefix as NumberFormat,
            innerTypeRef: inner.trim(),
          });
        }
        return { ...value, defined: sanitized };
      }
    }

    const next: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(value)) next[k] = walk(v, k);
    return next;
  };

  const idl: any = walk(rawIdl);
  idl.types = idl.types ?? [];
  for (const { sanitizedName } of seen.values()) {
    if (!idl.types.find((t: any) => t.name === sanitizedName)) {
      idl.types.push({
        name: sanitizedName,
        type: { kind: "struct", fields: [] },
      });
    }
  }

  return { idl, bindings: Array.from(seen.values()) };
}

/**
 * Builds the codama TypeNode that each sanitized SmallVec placeholder
 * should resolve to.  Returns a map of `sanitizedName -> TypeNode`, suitable
 * for feeding to `updateDefinedTypesVisitor`.
 */
export function smallVecTypeOverrides(
  bindings: readonly SmallVecBinding[]
): Record<string, { type: TypeNode }> {
  const out: Record<string, { type: TypeNode }> = {};
  for (const b of bindings) {
    out[b.sanitizedName] = {
      type: arrayTypeNode(
        innerTypeFor(b.innerTypeRef),
        prefixedCountNode(numberTypeNode(b.prefix))
      ),
    };
  }
  return out;
}

function sanitizedNameFor(prefix: string, inner: string): string {
  // SmallVec<u8, CompiledInstruction> -> SmallVecU8CompiledInstruction
  const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
  return `SmallVec${cap(prefix)}${cap(inner.trim())}`;
}

function innerTypeFor(inner: string): TypeNode {
  const trimmed = inner.trim();
  if (trimmed === "Pubkey" || trimmed === "publicKey") {
    return publicKeyTypeNode();
  }
  if (/^[ui]\d+$/.test(trimmed)) {
    return numberTypeNode(trimmed as NumberFormat);
  }
  // Defined type — codama camelCases anchor PascalCase names on its own,
  // so we just lower-case the first letter to match the link target.
  return definedTypeLinkNode(camelCase(trimmed));
}

function camelCase(s: string): string {
  return s.charAt(0).toLowerCase() + s.slice(1);
}
