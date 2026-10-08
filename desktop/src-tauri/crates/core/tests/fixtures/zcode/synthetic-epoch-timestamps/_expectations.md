# synthetic-epoch-timestamps expectations (synthetic)

<a id="synthetic-epoch-timestamps-期望全合成"></a>

Numeric completedAt values 1800000000000 milliseconds or 1800000000 seconds normalize to
milliseconds; values <1e11 are seconds. Native model-io used ISO strings, so these numeric cases are
defensive synthetic coverage.

See the manually calculated expectations in the header of tests/zcode_gaps_synthetic.rs.
