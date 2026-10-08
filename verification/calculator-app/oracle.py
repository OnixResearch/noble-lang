#!/usr/bin/env python3
"""Independent, bounded calculator application oracle. No Noble imports or evaluator reuse."""
import ast
from decimal import Decimal
from fractions import Fraction
import json
import re
import sys


DECIMAL = re.compile(r"(?<![\w.])(?:\d+\.\d+)(?![\w.])")


class Function:
    def __init__(self, parameters, body, captured):
        self.parameters = parameters
        self.body = body
        self.captured = captured.copy()


def expression(text, environment, depth=0):
    if len(text) > 256 or depth > 16:
        raise ValueError("oracle expression bound")
    translated = DECIMAL.sub(lambda match: f'F("{match.group()}")', text.replace('^', '**'))
    tree = ast.parse(translated, mode='eval')

    def evaluate(node, names, calls):
        if calls > 16:
            raise ValueError("oracle call bound")
        if isinstance(node, ast.Constant) and type(node.value) is int:
            return Fraction(node.value)
        if isinstance(node, ast.Name) and node.id in names and isinstance(names[node.id], Fraction):
            return names[node.id]
        if isinstance(node, ast.UnaryOp) and isinstance(node.op, (ast.UAdd, ast.USub)):
            operand = evaluate(node.operand, names, calls)
            return operand if isinstance(node.op, ast.UAdd) else -operand
        if isinstance(node, ast.BinOp):
            left = evaluate(node.left, names, calls)
            right = evaluate(node.right, names, calls)
            if isinstance(node.op, ast.Add):
                return left + right
            if isinstance(node.op, ast.Sub):
                return left - right
            if isinstance(node.op, ast.Mult):
                return left * right
            if isinstance(node.op, ast.Div):
                return left / right
            if isinstance(node.op, ast.Pow) and right.denominator == 1 and abs(right.numerator) <= 512:
                return left ** right.numerator
        if isinstance(node, ast.Call) and isinstance(node.func, ast.Name) and not node.keywords:
            if node.func.id == 'F' and len(node.args) == 1 and isinstance(node.args[0], ast.Constant):
                return Fraction(Decimal(node.args[0].value))
            target = names.get(node.func.id)
            if isinstance(target, Function) and len(target.parameters) == len(node.args):
                arguments = [evaluate(argument, names, calls) for argument in node.args]
                local = target.captured | dict(zip(target.parameters, arguments))
                return expression(target.body, local, calls + 1)
        raise ValueError('unsupported oracle expression')

    return evaluate(tree.body, environment, depth)


COMMANDS = [
    {'op': 'eval', 'expression': '2 + 3 * 4'},
    {'op': 'eval', 'expression': '(2 + 3) * 4'},
    {'op': 'eval', 'expression': '2^3^2'},
    {'op': 'eval', 'expression': '-2^2'},
    {'op': 'eval', 'expression': '0.1 + 0.2'},
    {'op': 'eval', 'expression': '9007199254740993 - 9007199254740992'},
    {'op': 'var', 'name': 'x', 'expression': '10'},
    {'op': 'function', 'name': 'f', 'parameters': ['y'], 'body': 'x + y'},
    {'op': 'var', 'name': 'x', 'expression': '20'},
    {'op': 'eval', 'expression': 'f(1)'},
    {'op': 'function', 'name': 'captured', 'parameters': ['n'], 'body': 'f(n)'},
    {'op': 'function', 'name': 'f', 'parameters': ['y'], 'body': '100 + y'},
    {'op': 'eval', 'expression': 'captured(1)'},
    {'op': 'eval', 'expression': 'f(1)'},
    {'op': 'var', 'name': 'x', 'expression': '1/0'},
    {'op': 'eval', 'expression': 'x'},
]


def expected():
    environment = {}
    answers = []
    for command in COMMANDS:
        try:
            if command['op'] == 'function':
                environment[command['name']] = Function(
                    command['parameters'], command['body'], environment)
                answers.append({'accepted': True, 'display': 'defined'})
                continue
            value = expression(command['expression'], environment)
            answers.append({'accepted': True,
                            'fraction': [str(value.numerator), str(value.denominator)]})
            if command['op'] == 'var':
                environment[command['name']] = value
        except ZeroDivisionError:
            answers.append({'error': {'category': 'DivisionByZero'}})
    return answers


def check(replies, count=None):
    controls = expected()[:count]
    if len(replies) != len(controls):
        raise AssertionError(f'oracle reply count {len(replies)} != {len(controls)}')
    for index, (actual, wanted) in enumerate(zip(replies, controls)):
        if 'error' in wanted:
            if actual.get('error', {}).get('category') != wanted['error']['category'] or actual.get('accepted'):
                raise AssertionError(f'application oracle error mismatch at {index}: {actual}')
        elif actual.get('accepted') is not True:
            raise AssertionError(f'application oracle admission mismatch at {index}: {actual}')
        elif 'fraction' in wanted and actual.get('fraction') != wanted['fraction']:
            raise AssertionError(f'application oracle exact result mismatch at {index}: {actual.get("fraction")} != {wanted["fraction"]}')
        elif 'display' in wanted and actual.get('display') != wanted['display']:
            raise AssertionError(f'application oracle definition mismatch at {index}: {actual}')
        if 'error' in wanted and index and actual.get('state_sha256') != replies[index - 1].get('state_sha256'):
            raise AssertionError(f'application oracle failed transition changed state at {index}')


def selfcheck():
    controls = expected()
    assert controls[0]['fraction'] == ['14', '1']
    assert controls[1]['fraction'] == ['20', '1']
    assert controls[2]['fraction'] == ['512', '1']
    assert controls[3]['fraction'] == ['-4', '1']
    assert controls[4]['fraction'] == ['3', '10']
    assert controls[5]['fraction'] == ['1', '1']
    assert controls[9]['fraction'] == ['11', '1']
    assert controls[12]['fraction'] == ['11', '1']
    assert controls[13]['fraction'] == ['101', '1']
    assert controls[14]['error']['category'] == 'DivisionByZero'
    assert controls[15]['fraction'] == ['20', '1']
    print(json.dumps({'outcome': 'independent-application-oracle-selfcheck', 'cases': len(COMMANDS)}))


if __name__ == '__main__':
    try:
        mode = sys.argv[1]
        if mode == '--cases' and len(sys.argv) == 2:
            print(json.dumps(COMMANDS))
        elif mode == '--check' and len(sys.argv) in (2, 3):
            check(json.load(sys.stdin), int(sys.argv[2]) if len(sys.argv) == 3 else None)
            print(json.dumps({'outcome': 'application-oracle-agreement'}))
        elif mode == '--selfcheck' and len(sys.argv) == 2:
            selfcheck()
        else:
            raise ValueError('usage: oracle.py --cases|--check [count]|--selfcheck')
    except (AssertionError, ValueError, TypeError, KeyError) as error:
        print(f'application oracle refused: {error}', file=sys.stderr)
        sys.exit(1)
