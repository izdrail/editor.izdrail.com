<x-card :title="{{ $title }}">
{{-- comment @if --}}
@if($user)
  <livewire:counter />{!! $html !!}
@endif
@php $x = '@endphp'; @endphp
@verbatim {{ not PHP }} @endverbatim
</x-card>
