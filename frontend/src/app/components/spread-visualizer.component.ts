import { Component, Input, Signal, computed } from '@angular/core';
import { CommonModule } from '@angular/common';

@Component({
  selector: 'app-spread-visualizer',
  standalone: true,
  imports: [CommonModule],
  template: `
    <div class="card">
      <div class="flex justify-between items-start mb-4">
        <h3 class="text-gray-400 text-sm uppercase tracking-wide">Spread</h3>
        <span 
          *ngIf="isStale()" 
          class="text-xs bg-red-500 text-white px-2 py-1 rounded">
          STALE
        </span>
      </div>
      
      <p [class]="spreadColorClass()" class="price-display">
        {{ spread() | number:'1.3-3' }}%
      </p>
      
      <div class="mt-4 pt-4 border-t border-gray-700">
        <div class="flex justify-between text-sm">
          <span class="text-gray-400">Latency</span>
          <span [class]="latencyColorClass()">
            {{ latency() }}ms
          </span>
        </div>
      </div>
    </div>
  `,
  styles: []
})
export class SpreadVisualizerComponent {
  @Input({ required: true }) spread!: Signal<number>;
  @Input({ required: true }) latency!: Signal<number>;
  @Input({ required: true }) isStale!: Signal<boolean>;
  
  spreadColorClass = computed(() => {
    const spreadValue = this.spread();
    if (Math.abs(spreadValue) > 0.05) {
      return spreadValue > 0 ? 'text-emerald-400' : 'text-red-400';
    }
    return 'text-gray-400';
  });
  
  latencyColorClass = computed(() => {
    const latencyValue = this.latency();
    if (latencyValue > 100) return 'text-red-400';
    if (latencyValue > 50) return 'text-yellow-400';
    return 'text-emerald-400';
  });
}
