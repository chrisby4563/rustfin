import json, re, sys
spec=json.load(open(sys.argv[1])); src=open(sys.argv[2]).read()
schemas=spec['components']['schemas']
def rust_enum(name):
    m=re.search(r'\n    pub enum '+name+r' \{\n(.*?)\n    \}\n', src, re.S)
    if not m: return None
    vals=[]; ren=None
    for line in m.group(1).split('\n'):
        l=line.strip()
        r=re.match(r'#\[serde\(rename = "(.*)"\)\]',l)
        if r: ren=r.group(1); continue
        v=re.match(r'([A-Za-z0-9_]+),?$',l)
        if v: vals.append(ren or v.group(1)); ren=None
    return vals
missing_total=0
for name,sc in sorted(schemas.items()):
    if 'enum' in sc and sc.get('type')=='string':
        have=rust_enum(name)
        if have is None: print("NEU (fehlt ganz):",name); continue
        miss=[v for v in sc['enum'] if v not in have]
        gone=[v for v in have if v not in sc['enum']]
        if miss or gone:
            missing_total+=len(miss); print(f"{name}: +{miss} -{gone}")
print("fehlende Werte gesamt:",missing_total)
# required-in-rust but optional on server
for name,sc in sorted(schemas.items()):
    if 'properties' not in sc: continue
    m=re.search(r'\n    pub struct '+name+r' \{\n(.*?)\n    \}\n', src, re.S)
    if not m: continue
    req=set(sc.get('required',[]))
    for fm in re.finditer(r'(?:#\[serde\(rename = "([^"]+)"[^\]]*\)\]\s*)?pub ([a-z0-9_]+): ([^,\n]+),', m.group(1)):
        ty=fm.group(3)
        if not ty.startswith('Option<') and 'default' not in m.group(0)[max(0,fm.start()-120):fm.start()]:
            jn=fm.group(1)
            if jn and (jn not in sc['properties'] or (jn not in req and sc['properties'][jn].get('nullable'))):
                print("PFLICHT?",name,jn,ty)
