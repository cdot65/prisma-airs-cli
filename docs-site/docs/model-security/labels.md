---
title: Labels
---

# Labels

Labels help organize and categorize model security scans with key-value metadata.

## Browse Label Taxonomy

### List Label Keys

```bash
airs model-security labels keys
```

### List Values for a Key

```bash
airs model-security labels values env
```

## Add Labels

```bash
airs model-security labels add <scanUuid> --labels '[{"key":"env","value":"prod"}]'
```

## Set Labels (Replace All)

```bash
airs model-security labels set <scanUuid> --labels '[{"key":"env","value":"staging"},{"key":"team","value":"ml"}]'
```

## Delete Labels

```bash
airs model-security labels delete <scanUuid> --keys env,team
```
