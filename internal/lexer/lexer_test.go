package lexer

import "testing"

func testTokenize(t *testing.T, source []byte, expected []Tag) {
	lexer := Init(source)
	for i, expectedTag := range expected {
		token := lexer.Next()
		if token == nil {
			t.Fatalf("Unexpected EOF at token %d", i)
		}
		if token.Tag != expectedTag {
			t.Fatalf("Token %d: expected %v, got %v", i, expectedTag, token.Tag)
		}
	}
	if lexer.Next() != nil {
		t.Fatalf("Expected EOF but got another token")
	}
}

func TestIdentifier(t *testing.T) {
	source := []byte("key1")
	lexer := Init(source)
	token := lexer.Next()
	if token == nil {
		t.Fatal("Expected token")
	}
	if string(token.GetValue(source)) != "key1" {
		t.Errorf("Expected 'key1', got '%s'", token.GetValue(source))
	}
	if token.Tag != identifier {
		t.Errorf("Expected identifier, got %v", token.Tag)
	}
}

func TestKeyEqualsValue(t *testing.T) {
	source := []byte("key = value")
	testTokenize(t, source, []Tag{identifier, equal, identifier})
}

func TestNumbers(t *testing.T) {
	source := []byte("key = 108")
	testTokenize(t, source, []Tag{identifier, equal, literal_number})
}

func TestDecimalNumber(t *testing.T) {
	source := []byte("value = 0.1")
	testTokenize(t, source, []Tag{identifier, equal, literal_number})
}

func TestStrings(t *testing.T) {
	source := []byte(`"test string"`)
	testTokenize(t, source, []Tag{literal_string})
}

func TestStringsNotTerminated(t *testing.T) {
	source := []byte(`"not terminated string`)
	testTokenize(t, source, []Tag{invalid})
}

func TestBooleans(t *testing.T) {
	source := []byte(`is_yes = yes
is_no = no`)
	testTokenize(t, source, []Tag{
		identifier, equal, literal_boolean,
		identifier, equal, literal_boolean,
	})
}

func TestBlocks(t *testing.T) {
	source := []byte(`limit = {
	age > 18
	}`)
	testTokenize(t, source, []Tag{
		identifier, equal, l_brace,
		identifier, greater_than, literal_number,
		r_brace,
	})
}

func TestScopeOperators(t *testing.T) {
	source := []byte(`key1 = title:k_france.capital
key2 = @some_var`)
	testTokenize(t, source, []Tag{
		// key1 = title:k_france.capital
		identifier, equal, identifier, colon, identifier, dot, identifier,
		// key2 = @some_var
		identifier, equal, script_value,
	})
}

func TestHyphenatedIdentifier(t *testing.T) {
	// CK3 title keys contain hyphens, e.g. c_anti-atlas.
	source := []byte(`c_anti-atlas = { x = 1 }`)
	testTokenize(t, source, []Tag{
		identifier, equal, l_brace,
		identifier, equal, literal_number,
		r_brace,
	})
}

func TestHyphenNotAbsorbedAsMinus(t *testing.T) {
	// A leading '-' before a value is still a minus (negative number),
	// and a spaced '-' is still subtraction — not part of an identifier.
	source := []byte(`key = -1
diff = a - b`)
	testTokenize(t, source, []Tag{
		// key = -1
		identifier, equal, minus, literal_number,
		// diff = a - b
		identifier, equal, identifier, minus, identifier,
	})
}

func TestDotFloats(t *testing.T) {
	// CK3 color/position lists use leading- and trailing-dot floats.
	source := []byte(`color = { .7 0.05 1. }`)
	testTokenize(t, source, []Tag{
		identifier, equal, l_brace,
		literal_number, literal_number, literal_number,
		r_brace,
	})
}

func TestDateLiteral(t *testing.T) {
	// Y.M.D dates are a single token, not number-dot-number.
	source := []byte(`creation_date = 1099.1.1`)
	testTokenize(t, source, []Tag{
		identifier, equal, literal_date,
	})
}

func TestDotStillScopeOperator(t *testing.T) {
	// A '.' between identifiers remains a scope operator, not a number.
	source := []byte(`x = title:k_france.capital`)
	testTokenize(t, source, []Tag{
		identifier, equal, identifier, colon, identifier, dot, identifier,
	})
}

func TestScriptValues(t *testing.T) {
	source := []byte(`@my_const = 0.15
key = @my_const
neg = @[my_const * -1]`)
	testTokenize(t, source, []Tag{
		// @my_const = 0.15
		script_value, equal, literal_number,
		// key = @my_const
		identifier, equal, script_value,
		// neg = @[my_const * -1]
		identifier, equal, script_math,
	})
}

func TestGreaterLessEqualOperators(t *testing.T) {
	source := []byte(`age > 18
age < 18
age >= 18
age <= 18`)
	testTokenize(t, source, []Tag{
		identifier, greater_than, literal_number,
		identifier, less_than, literal_number,
		identifier, greater_equal, literal_number,
		identifier, less_equal, literal_number,
	})
}

func TestDifferentEqualOperators(t *testing.T) {
	source := []byte(`capital = c_france
age == 18
age != 18
this ?= c_france`)
	testTokenize(t, source, []Tag{
		identifier, equal, identifier,
		identifier, equal_equal, literal_number,
		identifier, not_equal, literal_number,
		identifier, question_equal, identifier,
	})
}

func TestInvalidTokens(t *testing.T) {
	source := []byte(`something!
something?`)
	testTokenize(t, source, []Tag{
		identifier, invalid,
		identifier, invalid,
	})
}

func TestIdentifierCanStartFromNumber(t *testing.T) {
	source := []byte("8_something")
	testTokenize(t, source, []Tag{identifier})
}

func TestUTF8BOM(t *testing.T) {
	source := append([]byte(UTF8_BOM_SEQUENCE), []byte("key = value")...)
	testTokenize(t, source, []Tag{identifier, equal, identifier})
}

func TestSkippingBOMDoesNotBreakTokenGetValue(t *testing.T) {
	source := append([]byte(UTF8_BOM_SEQUENCE), []byte("key = value")...)
	lexer := Init(source)
	token := lexer.Next()
	if token == nil {
		t.Fatal("Expected token")
	}
	if token.Start != 3 { // BOM - 3 bytes offset
		t.Errorf("Expected start position 3, got %d", token.Start)
	}
	if token.End != 6 { // "key" ends at position 6 (3 BOM + 3 for "key")
		t.Errorf("Expected end position 6, got %d", token.End)
	}
	value := token.GetValue(source)
	if string(value) != "key" {
		t.Errorf("Expected 'key', got '%s'", value)
	}
}

func TestSkipComments(t *testing.T) {
	source := []byte(`key = value # something commented
key = value`)
	testTokenize(t, source, []Tag{identifier, equal, identifier, identifier, equal, identifier})
}

func TestIdentifierCanContainAmpersand(t *testing.T) {
	source := []byte("ghw_region_finland_&_estonia = something")
	testTokenize(t, source, []Tag{identifier, equal, identifier})
}

func TestUTF8Identifier(t *testing.T) {
	source := []byte("flag:Linnéa José")
	testTokenize(t, source, []Tag{identifier, colon, identifier, identifier})
}

func TestUTF8IdentifierValues(t *testing.T) {
	source := []byte("flag:Linnéa José")
	l := Init(source)

	expected := []string{"flag", ":", "Linnéa", "José"}
	for _, want := range expected {
		tok := l.Next()
		if tok == nil {
			t.Fatalf("unexpected EOF, wanted %q", want)
		}
		if got := string(tok.GetValue(source)); got != want {
			t.Errorf("got %q, want %q", got, want)
		}
	}
	if l.Next() != nil {
		t.Fatal("expected EOF")
	}
}

func TestMacroParam(t *testing.T) {
	testTokenize(t, []byte(`$CHILD$`), []Tag{macro_param})
	testTokenize(t, []byte(`exists = $CHILD$`), []Tag{identifier, equal, macro_param})
	testTokenize(t, []byte(`$CHILD$ = {`), []Tag{macro_param, equal, l_brace})
	testTokenize(t, []byte(`$CHILD$.host`), []Tag{macro_param, dot, identifier})
	testTokenize(t, []byte(`$`), []Tag{dollar})
	testTokenize(t, []byte(`$$`), []Tag{dollar, dollar})

	src := []byte(`$CHILD$`)
	tok := Init(src).Next()
	if got := string(tok.GetValue(src)); got != "$CHILD$" {
		t.Fatalf("expected $CHILD$, got %q", got)
	}
}

func TestUTF8StringLiteral(t *testing.T) {
	source := []byte(`flag = "Linnéa José"`)
	testTokenize(t, source, []Tag{identifier, equal, literal_string})
}

func TestUTF8StringValue(t *testing.T) {
	source := []byte(`flag = "Linnéa José"`)
	l := Init(source)
	l.Next() // flag
	l.Next() // =
	tok := l.Next()
	if tok == nil || tok.Tag != literal_string {
		t.Fatal("expected literal_string")
	}
	if got := string(tok.GetValue(source)); got != `"Linnéa José"` {
		t.Errorf("got %q, want %q", got, `"Linnéa José"`)
	}
}
