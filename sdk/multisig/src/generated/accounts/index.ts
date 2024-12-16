export * from './Batch'
export * from './BatchTransaction'
export * from './ProgramConfig'
export * from './Proposal'
export * from './Settings'
export * from './SettingsTransaction'
export * from './SpendingLimit'
export * from './Transaction'
export * from './TransactionBuffer'

import { Batch } from './Batch'
import { BatchTransaction } from './BatchTransaction'
import { ProgramConfig } from './ProgramConfig'
import { Proposal } from './Proposal'
import { SettingsTransaction } from './SettingsTransaction'
import { Settings } from './Settings'
import { SpendingLimit } from './SpendingLimit'
import { TransactionBuffer } from './TransactionBuffer'
import { Transaction } from './Transaction'

export const accountProviders = {
  Batch,
  BatchTransaction,
  ProgramConfig,
  Proposal,
  SettingsTransaction,
  Settings,
  SpendingLimit,
  TransactionBuffer,
  Transaction,
}
