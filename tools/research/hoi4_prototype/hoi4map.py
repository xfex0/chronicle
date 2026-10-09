import math, yaml, pickle
cfg=yaml.safe_load(open(__import__('pathlib').Path(__file__).resolve().parents[3] / 'games/hoi4/semantic_mapping.yaml'))
def civ_from(d):
    rows,fact,facs=d['rows'],d['fact'],d['facs']
    ic={t:f[0]+f[1] for t,f in fact.items() if t in rows and f[0]+f[1]>0}
    tot=sum(ic.values()); w={t:v/tot for t,v in ic.items()}
    rw=cfg['ruling_party_weight']
    def mix(ind, r):
        iv=cfg['ideology_values'][ind]; pop=r['pop']; s=sum(pop.values()) or 1
        pm=sum(pop.get(p,0)/s*iv[p] for p in iv)
        return rw*iv.get(r['ruling'],0.5)+(1-rw)*pm
    def law(r, table, default):
        for i in r['ideas']:
            if i in table: return table[i]
        return default
    at_war=set()
    for t,r in rows.items():
        for a,b in r.get('war_pairs',[]): at_war.update([a,b])
    v={}
    v['authoritarianism']=sum(w[t]*mix('authoritarianism',rows[t]) for t in w)
    v['social_equality']=sum(w[t]*mix('social_equality',rows[t]) for t in w)
    v['xenophobia']=sum(w[t]*min(1,max(0,mix('xenophobia',rows[t])+law(rows[t],cfg['trade_law_xenophobia'],0))) for t in w)
    m=cfg['militarism']
    v['militarism']=sum(w[t]*(m['military_factory_share']*fact[t][1]/max(1,ic[t]) + m['war_support']*rows[t]['war_support'] + m['conscription']*law(rows[t],cfg['conscription_law_level'],0.1)) for t in w)
    v['global_wars']=sum(w[t] for t in w if t in at_war)
    nukes=sum(r['nukes'] for r in rows.values()); v['nuclear_weapons']=1-math.exp(-nukes/cfg['nukes_scale'])
    states=sum(f[3] for t,f in fact.items()); v['industrialization']=min(1,tot/max(1,states)/cfg['factories_per_state_reference'])
    v['technology']=min(1,sum(w[t]*rows[t]['ntech'] for t in w)/cfg['tech_reference_count'])
    sp=sum(w[t]*sum(1 for x in rows[t]['techs'] if any(mk in x for mk in cfg['space_tech_markers'])) for t in w)
    v['space_program']=min(1,sp/cfg['space_tech_reference'])
    e=cfg['economic_planning']
    v['economic_planning']=sum(w[t]*(e['communist_rule']*(rows[t]['ruling']=='communism')+e['economy_law']*law(rows[t],cfg['economy_law_level'],0.1)) for t in w)
    fshare=[(name,ide,sum(w.get(m_,0) for m_ in mem)) for name,ide,mem in facs]
    largest=max(fshare,key=lambda x:x[2]) if fshare else (None,None,0)
    v['planetary_unification']=largest[2]
    c=cfg['cooperation']; dem=sum(w[t] for t in w if rows[t]['ruling']=='democratic')
    v['international_cooperation']=c['peace_share']*(1-v['global_wars'])+c['democratic_share']*dem+c['largest_faction_share']*largest[2]
    blocs=sum(1 for f in fshare if f[2]>=cfg['bloc_min_share'])
    ide_map={'fascism':'fascism','communism':'communism','democratic':'democracy','neutrality':'non_aligned'}
    ideology=ide_map.get(largest[1] if largest[2]>=cfg['bloc_min_share'] else rows[max(w,key=w.get)]['ruling'],'non_aligned')
    return dict(v), max(1,min(6,blocs if blocs else 1)), ideology, fshare
