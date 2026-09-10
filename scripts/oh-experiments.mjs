import { calibrationRecords, clueTransferRecords, validateCalibration } from "./oh-ledger.mjs";
import { validateClueTransfer } from "./oh-transfer-report.mjs";
import { validateIndexedTransfer } from "./oh-indexed-report.mjs";
import { validateImplicationCalibration } from "./oh-implication-report.mjs";
import { validateFragmentInterface } from "./oh-fragment-report.mjs";
import { fragmentInterfaceRecords, implicationCalibrationRecords, indexedTransferRecords } from "./oh-experiment-records.mjs";

// This is an explicit publication allowlist, not a plugin or arbitrary-ingest
// interface. Keep historical validators and constructors when adding a version.
export const EXPERIMENTS = [
  {
    id: "unit-propagation-calibration-v1",
    path: "artifacts/calibration.json",
    activityPrefix: "activity:calibration-",
    validate: validateCalibration,
    records: calibrationRecords,
  },
  {
    id: "clue-transfer-v1",
    path: "artifacts/clue-transfer.json",
    protocolPath: "experiments/clue-transfer-protocol.md",
    activityPrefix: "activity:clue-transfer-",
    validate: validateClueTransfer,
    records: clueTransferRecords,
  },
  {
    id: "indexed-transfer-v1",
    path: "artifacts/indexed-transfer.json",
    protocolPath: "experiments/indexed-transfer-protocol.md",
    activityPrefix: "activity:indexed-transfer-",
    validate: validateIndexedTransfer,
    records: indexedTransferRecords,
  },
  {
    id: "implication-calibration-v1",
    path: "artifacts/implication-calibration.json",
    protocolPath: "experiments/implication-protocol.md",
    activityPrefix: "activity:implication-calibration-",
    validate: validateImplicationCalibration,
    records: implicationCalibrationRecords,
  },
  {
    id: "fragment-interface-v1",
    path: "artifacts/fragment-interface.json",
    protocolPath: "experiments/fragment-interface-protocol.md",
    activityPrefix: "activity:fragment-interface-",
    validate: validateFragmentInterface,
    records: fragmentInterfaceRecords,
  },
];

export function experimentById(id) { return EXPERIMENTS.find(experiment => experiment.id === id); }
export function experimentByPath(path) { return EXPERIMENTS.find(experiment => experiment.path === path); }
