#!/bin/sh
# Enchainement des mesures hors ligne du croisement et de l'elagage (aucun appel au modele).
# 1. attend la fin du premier banc (croisement d'avant correction) ;
# 2. recompile le banc avec le croisement corrige ;
# 3. attend la fin du tir GLM (processeur libre) ;
# 4. elagage des 24 champions Gemini, puis second banc (croisement corrige).
set -u
cd /c/Devs/nomad-ast
M=mesures
G=C:/Devs/nomad/results_gemini_courbe/evolution
SEEDS24=$(for r in $(seq 0 23); do echo $((8000 + 17 * r)); done)
SEEDS12=$(for r in $(seq 0 11); do echo $((8000 + 17 * r)); done)
export NOMAD_BP_PROFILE=weibull NOMAD_FUEL_LIMIT=400000
log() { echo "$(date +%H:%M) $*" | tee -a $M/apres_tir.log; }

while tasklist | grep -qi crossover_probe; do sleep 60; done
log "banc 1 termine : $(wc -l < $M/crossover_probe.jsonl) enfants"

cargo build -j 4 --release -p nomad-cli --example crossover_probe > $M/build.log 2>&1 \
  || { log "ECHEC compilation, voir build.log"; exit 1; }
log "banc recompile avec le croisement corrige"

while tasklist | grep -qi nomad-gemini-courbe; do sleep 120; done
log "tir GLM termine, processeur libre"

./target/release/examples/prune_champions.exe $G $SEEDS24 > $M/prune.jsonl 2> $M/prune.err.log
log "elagage termine : $(wc -l < $M/prune.jsonl) champions"

./target/release/examples/crossover_probe.exe $G 8 3 $SEEDS12 > $M/crossover_probe_v2.jsonl 2> $M/crossover_probe_v2.err.log
log "banc 2 termine : $(wc -l < $M/crossover_probe_v2.jsonl) enfants"
log "FIN"
