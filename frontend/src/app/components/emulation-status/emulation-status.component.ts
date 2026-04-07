import { Component, computed } from '@angular/core';
import { CommonModule } from '@angular/common';
import { EmulationDataService } from '../../services/emulation-data.service';

@Component({
  selector: 'app-emulation-status',
  standalone: true,
  imports: [CommonModule],
  templateUrl: './emulation-status.component.html',
  styleUrls: ['./emulation-status.component.css']
})
export class EmulationStatusComponent {
  // Computed values - direct access to service
  status = computed(() => this.emulationDataService.emulationStatus());
  isLoading = computed(() => this.emulationDataService.isLoading());
  error = computed(() => this.emulationDataService.error());

  // Computed session time remaining
  sessionTimeRemaining = computed(() => {
    const status = this.status();
    if (!status?.session_expires_at) return null;

    const expiresAt = new Date(status.session_expires_at);
    const now = new Date();
    const diff = expiresAt.getTime() - now.getTime();

    if (diff <= 0) return 'Expired';

    const hours = Math.floor(diff / (1000 * 60 * 60));
    const minutes = Math.floor((diff % (1000 * 60 * 60)) / (1000 * 60));

    return `${hours}h ${minutes}m`;
  });

  // Mode indicator color
  modeColor = computed(() => {
    const mode = this.status()?.trading_mode;
    return mode === 'Hybrid' ? 'text-blue-400' : 'text-red-400';
  });

  // Session status color
  sessionColor = computed(() => {
    return this.status()?.session_valid ? 'text-green-400' : 'text-red-400';
  });

  constructor(private emulationDataService: EmulationDataService) {}

  refresh(): void {
    this.emulationDataService.refresh();
  }

  formatDate(dateStr: string | null): string {
    if (!dateStr) return 'N/A';
    return new Date(dateStr).toLocaleString();
  }
}
