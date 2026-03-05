import { Component, Input, Signal } from '@angular/core';
import { CommonModule } from '@angular/common';

@Component({
  selector: 'app-price-card',
  standalone: true,
  imports: [CommonModule],
  template: `
    <div class="card">
      <h3 class="text-gray-400 text-sm uppercase tracking-wide mb-2">{{ exchange }}</h3>
      <p class="price-display text-white">
        {{ price() | number:'1.2-2' }}
      </p>
      <p class="text-gray-500 text-xs mt-1">BTC/USDT</p>
    </div>
  `,
  styles: []
})
export class PriceCardComponent {
  @Input({ required: true }) exchange!: string;
  @Input({ required: true }) price!: Signal<number>;
}
