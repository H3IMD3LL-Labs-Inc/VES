# Overview

```
SourcePayload
   ↓
format.rs        → detect format
   ↓
parsers.rs       → parse into structured data
   ↓
normalizer.rs    → enforce canonical schema
   ↓
types.rs         → Provides LogEvent
```

`/processor` is not a pipeline tool, it is a deterministic signal extractor for a
reasoning system.

---

# What is the Parser
The parser defines the canonical schema the HEIMDELL Server depends on. It is the
boundary between entropy(`raw_data`) and structure(*graph-ready data*)

Therefore, the parser *MUST BE*:
1. Deterministic:
   - Same input -> same output, *ALWAYS*
2. Loss-aware(not lossless, not lossy):
   - Keep `raw_bytes` message
   - Extract what's confidently extractable
3. Conservative:
   - Don't "guess meaning"
   - Only extract structure
4. Failure-tolerant:
   - Never allow pipeline to break
   - Always emit something usable

Also, the parser *IS NOT*:
1. A "smart inference system"
2. A "user-controlled pipeline"
3. Responsible for meaning

*Core Design Philosophy*
- Specialization > Generalization
- Determinism > Flexibility
- Stability > Cleverness

