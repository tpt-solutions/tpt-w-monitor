// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

/** Props every dashboard widget receives from the grid or the overlay host. */
export interface WidgetProps {
  /** Removes the widget from the dashboard (or closes the overlay). */
  onRemove: () => void;
  /** Pops the widget out into its own window. Absent when that is not possible. */
  onPopOut?: () => void;
}
